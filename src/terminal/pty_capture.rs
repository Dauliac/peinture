//! PTY capture — spawn a command in a pseudo-terminal and capture its
//! rendered screen output via a virtual terminal emulator.
//!
//! This allows peinture to overlay its beacon on top of any program that
//! rewrites the screen (e.g. `nix-output-monitor`, `htop`, `cargo watch`).
//!
//! The captured program's screen is interpreted by `vt100` into a virtual
//! framebuffer. Each frame, the current screen content is extracted as
//! ANSI-formatted lines and can be combined with a peinture beacon in a
//! single `Painter::render_frame()` call.
//!
//! # Example
//!
//! ```rust,ignore
//! use peinture::terminal::pty_capture::PtyCapture;
//!
//! let mut cap = PtyCapture::spawn("nom", &["--", "nix", "build"], 20, 80)?;
//! while cap.process_available() {
//!     let lines = cap.screen_lines();
//!     // combine with beacon, render via Painter
//! }
//! ```

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::sync::mpsc;
use std::thread;

/// A captured PTY process whose screen output is interpreted by a virtual
/// terminal emulator (`vt100`).
pub struct PtyCapture {
    parser: vt100::Parser,
    rx: mpsc::Receiver<Vec<u8>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// Keep master alive so the PTY pair stays connected.
    _master: Box<dyn portable_pty::MasterPty + Send>,
    finished: bool,
    rows: u16,
    cols: u16,
}

impl PtyCapture {
    /// Spawn a command in a PTY with the given virtual screen size.
    ///
    /// `program` is the executable, `args` are its arguments.
    /// `rows` and `cols` define the virtual terminal dimensions — the captured
    /// program will believe it has a terminal of this size.
    pub fn spawn(program: &str, args: &[&str], rows: u16, cols: u16) -> std::io::Result<Self> {
        let mut cmd = CommandBuilder::new(program);
        for arg in args {
            cmd.arg(*arg);
        }
        Self::spawn_command(cmd, rows, cols)
    }

    /// Spawn a pre-built `CommandBuilder` in a PTY.
    pub fn spawn_command(
        cmd: CommandBuilder,
        rows: u16,
        cols: u16,
    ) -> std::io::Result<Self> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

        // Drop slave handle — the child process holds its own fd.
        drop(pair.slave);

        let (tx, rx) = mpsc::channel();

        thread::spawn(move || {
            reader_loop(reader, tx);
        });

        let parser = vt100::Parser::new(rows, cols, 0);

        Ok(Self {
            parser,
            rx,
            child,
            _master: pair.master,
            finished: false,
            rows,
            cols,
        })
    }

    /// Drain available PTY output and feed it to the virtual terminal.
    ///
    /// Returns `true` if the child process is still running.
    pub fn process_available(&mut self) -> bool {
        while let Ok(bytes) = self.rx.try_recv() {
            self.parser.process(&bytes);
        }

        if !self.finished {
            if let Ok(Some(_)) = self.child.try_wait() {
                // Drain any remaining bytes after child exits.
                while let Ok(bytes) = self.rx.try_recv() {
                    self.parser.process(&bytes);
                }
                self.finished = true;
            }
        }

        !self.finished
    }

    /// Get the current virtual screen as ANSI-formatted lines.
    ///
    /// Each line contains ANSI color/style escape sequences matching what the
    /// captured program rendered. Trailing whitespace is trimmed per line.
    pub fn screen_lines(&self) -> Vec<String> {
        let screen = self.parser.screen();
        (0..self.rows)
            .map(|row| row_to_ansi(screen, row, self.cols))
            .collect()
    }

    /// Get the current virtual screen as plain text lines (no ANSI codes).
    pub fn screen_lines_plain(&self) -> Vec<String> {
        let contents = self.parser.screen().contents();
        contents.lines().map(String::from).collect()
    }

    /// Number of non-empty lines on the virtual screen.
    ///
    /// Useful for trimming the captured output to only the portion the
    /// child actually wrote to.
    pub fn active_rows(&self) -> usize {
        let screen = self.parser.screen();
        let mut last_nonempty = 0;
        for row in 0..self.rows {
            for col in 0..self.cols {
                if let Some(cell) = screen.cell(row, col) {
                    if !cell.contents().is_empty() && cell.contents() != " " {
                        last_nonempty = row as usize + 1;
                        break;
                    }
                }
            }
        }
        last_nonempty
    }

    /// Whether the child process has exited.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Wait for the child to exit and return its exit status.
    pub fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
        let status = self.child.wait()?;
        // Drain remaining output.
        while let Ok(bytes) = self.rx.try_recv() {
            self.parser.process(&bytes);
        }
        self.finished = true;
        Ok(status)
    }

    /// Virtual screen rows.
    pub fn rows(&self) -> u16 {
        self.rows
    }

    /// Virtual screen columns.
    pub fn cols(&self) -> u16 {
        self.cols
    }
}

// ─── Reader thread ───────────────────────────────────────────────────────

fn reader_loop(mut reader: Box<dyn Read + Send>, tx: mpsc::Sender<Vec<u8>>) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if tx.send(buf[..n].to_vec()).is_err() {
                    break;
                }
            }
        }
    }
}

// ─── ANSI rendering from vt100 cells ─────────────────────────────────────

fn row_to_ansi(screen: &vt100::Screen, row: u16, cols: u16) -> String {
    let mut out = String::with_capacity(cols as usize * 2);
    let mut prev = CellAttrs::default();

    for col in 0..cols {
        let Some(cell) = screen.cell(row, col) else {
            out.push(' ');
            continue;
        };
        let attrs = CellAttrs::from_cell(cell);
        if attrs != prev {
            out.push_str(&attrs.sgr_sequence());
            prev = attrs;
        }
        let contents = cell.contents();
        if contents.is_empty() {
            out.push(' ');
        } else {
            out.push_str(&contents);
        }
    }

    // Reset if we emitted any styling.
    if prev != CellAttrs::default() {
        out.push_str("\x1b[0m");
    }

    // Trim trailing spaces (but keep ANSI reset).
    let trimmed = out.trim_end_matches(' ');
    if trimmed.is_empty() {
        String::new()
    } else if trimmed.len() < out.len() {
        let mut s = trimmed.to_string();
        // Re-append reset if it was trimmed.
        if !s.ends_with("\x1b[0m") && prev != CellAttrs::default() {
            s.push_str("\x1b[0m");
        }
        s
    } else {
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct CellAttrs {
    fg: ColorAttr,
    bg: ColorAttr,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ColorAttr {
    #[default]
    Default,
    Idx(u8),
    Rgb(u8, u8, u8),
}

impl CellAttrs {
    fn from_cell(cell: &vt100::Cell) -> Self {
        Self {
            fg: color_from_vt100(cell.fgcolor()),
            bg: color_from_vt100(cell.bgcolor()),
            bold: cell.bold(),
            dim: false,
            italic: cell.italic(),
            underline: cell.underline(),
            inverse: cell.inverse(),
        }
    }

    fn sgr_sequence(&self) -> String {
        let mut params: Vec<String> = vec!["0".into()]; // reset first

        if self.bold {
            params.push("1".into());
        }
        if self.dim {
            params.push("2".into());
        }
        if self.italic {
            params.push("3".into());
        }
        if self.underline {
            params.push("4".into());
        }
        if self.inverse {
            params.push("7".into());
        }

        if let Some(fg) = color_sgr(self.fg, true) {
            params.push(fg);
        }
        if let Some(bg) = color_sgr(self.bg, false) {
            params.push(bg);
        }

        format!("\x1b[{}m", params.join(";"))
    }
}

fn color_from_vt100(c: vt100::Color) -> ColorAttr {
    match c {
        vt100::Color::Default => ColorAttr::Default,
        vt100::Color::Idx(i) => ColorAttr::Idx(i),
        vt100::Color::Rgb(r, g, b) => ColorAttr::Rgb(r, g, b),
    }
}

fn color_sgr(c: ColorAttr, is_fg: bool) -> Option<String> {
    let base: u8 = if is_fg { 30 } else { 40 };
    match c {
        ColorAttr::Default => None,
        ColorAttr::Idx(i) if i < 8 => Some(format!("{}", base + i)),
        ColorAttr::Idx(i) if i < 16 => Some(format!("{}", base + 60 + (i - 8))),
        ColorAttr::Idx(i) => {
            let prefix = if is_fg { 38 } else { 48 };
            Some(format!("{prefix};5;{i}"))
        }
        ColorAttr::Rgb(r, g, b) => {
            let prefix = if is_fg { 38 } else { 48 };
            Some(format!("{prefix};2;{r};{g};{b}"))
        }
    }
}
