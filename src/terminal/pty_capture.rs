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
//! Lines that scroll off the top of the virtual terminal are captured via
//! the `vt100` scrollback buffer and can be drained with
//! [`PtyCapture::drain_scrollback()`] to feed into
//! [`Painter::stream_line()`](super::painter::Painter::stream_line).
//!
//! # Layout
//!
//! ```text
//! ─── top of real terminal ───
//! scrolled-off lines           ← drain_scrollback() → stream_line()
//! ─── top of virtual window ──
//! captured program screen      ← screen_lines() → pinned region
//! ─── bottom of virtual window
//! notification center + beacon ← beacon render  → pinned region
//! ─── bottom of real terminal ─
//! ```

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Read;
use std::sync::mpsc;
use std::thread;

/// Large scrollback buffer — rows that scroll off the top of the virtual
/// terminal are kept here so `drain_scrollback()` can retrieve them.
const SCROLLBACK_LEN: usize = 5000;

/// A captured PTY process whose screen output is interpreted by a virtual
/// terminal emulator (`vt100`).
pub struct PtyCapture {
    parser: vt100::Parser,
    rx: mpsc::Receiver<Vec<u8>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    finished: bool,
    rows: u16,
    cols: u16,
    /// How many scrollback rows we've already drained.
    scrollback_seen: usize,
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

        let parser = vt100::Parser::new(rows, cols, SCROLLBACK_LEN);

        Ok(Self {
            parser,
            rx,
            child,
            master: pair.master,
            finished: false,
            rows,
            cols,
            scrollback_seen: 0,
        })
    }

    /// Drain available PTY output and feed it to the virtual terminal.
    ///
    /// Returns `true` if the child process is still running.
    pub fn process_available(&mut self) -> bool {
        self.process_available_raw().0
    }

    /// Drain available PTY output, feed it to the virtual terminal, and
    /// return the raw bytes for direct relay to the real terminal.
    ///
    /// Use this when you want to relay the captured program's output
    /// directly (e.g. into a scroll region) while still keeping the
    /// vt100 screen state up to date.
    ///
    /// Returns `(still_running, raw_bytes)`.
    pub fn process_available_raw(&mut self) -> (bool, Vec<u8>) {
        let mut raw = Vec::new();
        while let Ok(bytes) = self.rx.try_recv() {
            raw.extend_from_slice(&bytes);
            self.parser.process(&bytes);
        }

        if !self.finished {
            if let Ok(Some(_)) = self.child.try_wait() {
                while let Ok(bytes) = self.rx.try_recv() {
                    raw.extend_from_slice(&bytes);
                    self.parser.process(&bytes);
                }
                self.finished = true;
            }
        }

        (!self.finished, raw)
    }

    /// Drain lines that have scrolled off the top of the virtual terminal.
    ///
    /// Returns ANSI-formatted lines suitable for
    /// [`Painter::stream_line()`](super::painter::Painter::stream_line).
    /// Call this every frame *after* [`process_available()`](Self::process_available).
    pub fn drain_scrollback(&mut self) -> Vec<String> {
        // Probe how many rows are in the scrollback buffer.
        // set_scrollback(MAX) clamps to scrollback.len().
        self.parser.set_scrollback(usize::MAX);
        let total = self.parser.screen().scrollback();
        self.parser.set_scrollback(0);

        if total <= self.scrollback_seen {
            return vec![];
        }

        let new_count = total - self.scrollback_seen;
        // We can safely set offset up to self.rows (screen height).
        // With offset=N, visible_rows() returns: last N scrollback rows +
        // first (screen_rows - N) screen rows. N must be <= screen_rows.
        let safe_offset = new_count.min(self.rows as usize);

        self.parser.set_scrollback(safe_offset);

        // The first `safe_offset` items from rows_formatted() are the
        // newest scrollback rows. We want them all (they are the new ones
        // that weren't in the buffer last time we looked).
        let lines: Vec<String> = self
            .parser
            .screen()
            .rows_formatted(0, self.cols)
            .take(safe_offset)
            .map(|bytes| {
                let s = String::from_utf8_lossy(&bytes);
                s.trim_end().to_string()
            })
            .collect();

        self.parser.set_scrollback(0);
        self.scrollback_seen = total;

        lines
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

    /// Number of non-empty rows on the virtual screen (from the top).
    ///
    /// Useful for compact rendering: only show the rows the captured
    /// program actually wrote to, so the beacon follows the content.
    pub fn active_rows(&self) -> usize {
        let screen = self.parser.screen();
        let mut last_nonempty = 0;
        for row in 0..self.rows {
            for col in 0..self.cols {
                if let Some(cell) = screen.cell(row, col) {
                    let c = cell.contents();
                    if !c.is_empty() && c != " " {
                        last_nonempty = row as usize + 1;
                        break;
                    }
                }
            }
        }
        last_nonempty
    }

    /// Get the current virtual screen as plain text lines (no ANSI codes).
    pub fn screen_lines_plain(&self) -> Vec<String> {
        let contents = self.parser.screen().contents();
        contents.lines().map(String::from).collect()
    }

    /// Resize the virtual terminal and notify the child process (SIGWINCH).
    ///
    /// Call this when the real terminal size changes.
    pub fn resize(&mut self, rows: u16, cols: u16) {
        if rows == self.rows && cols == self.cols {
            return;
        }
        self.rows = rows;
        self.cols = cols;
        self.parser.set_size(rows, cols);
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }

    /// Whether the child process has exited.
    pub fn is_finished(&self) -> bool {
        self.finished
    }

    /// Wait for the child to exit and return its exit status.
    pub fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> {
        let status = self.child.wait()?;
        while let Ok(bytes) = self.rx.try_recv() {
            self.parser.process(&bytes);
        }
        self.finished = true;
        Ok(status)
    }

    /// Current cursor row in the virtual terminal (0-based).
    pub fn cursor_row(&self) -> u16 {
        self.parser.screen().cursor_position().0
    }

    /// Returns `true` if the program needs more rows.
    ///
    /// Checks whether the cursor is near the bottom of the virtual
    /// terminal or the content has filled the available space.
    pub fn needs_grow(&self) -> bool {
        let cursor_row = self.cursor_row();
        let active = self.active_rows() as u16;
        // Cursor at last row, or content fills the screen.
        cursor_row + 1 >= self.rows || active >= self.rows
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
        // Wide characters (e.g. ⏱, emoji) occupy 2 cells in the vt100 grid.
        // The second cell is a "continuation" with no content. Skip it —
        // the terminal already advances the cursor by 2 for the wide char.
        if cell.is_wide_continuation() {
            continue;
        }
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
