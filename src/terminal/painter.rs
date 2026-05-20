//! Painter — nom-style terminal renderer.
//!
//! Follows the exact algorithm from nix-output-monitor:
//! 1. Begin synchronized update (\x1b[?2026h)
//! 2. Go to column 0 (if previous output was 1 line)
//! 3. Clear current line
//! 4. Move up + clear, repeated for each previous line
//! 5. Write all new content (stream lines + beacon lines)
//! 6. End synchronized update (\x1b[?2026l)
//! 7. Single write() syscall for the entire frame
//!
//! `printed_lines` only tracks the BEACON line count.
//! Stream lines scroll into scrollback and are not tracked.

use console::Term;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use super::sync_update::{SYNC_BEGIN, SYNC_END};

static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

pub struct Painter {
    term: Term,
    /// Number of BEACON lines printed last frame (not stream lines).
    printed_lines: usize,
    /// Pending stream lines.
    stream_buffer: Vec<String>,
    cursor_hidden: bool,
    term_width: u16,
}

impl Painter {
    pub fn new(term_width: u16) -> Self {
        Self {
            term: Term::stderr(),
            printed_lines: 0,
            stream_buffer: Vec::new(),
            cursor_hidden: false,
            term_width,
        }
    }

    pub fn stream_line(&mut self, line: String) {
        self.stream_buffer.push(line);
    }

    pub fn stream_lines(&mut self, lines: impl IntoIterator<Item = String>) {
        self.stream_buffer.extend(lines);
    }

    /// Render a frame — nom algorithm, single write syscall.
    pub fn render_frame(&mut self, beacon_lines: &[String]) {
        let mut buf = Vec::<u8>::with_capacity(4096);

        // ── Begin synchronized update ──
        buf.extend_from_slice(SYNC_BEGIN.as_bytes());

        // ── Phase 1: Move cursor to first beacon line ──
        // Only go up by the OLD printed_lines count — never touch stream content above.
        if self.printed_lines == 1 {
            buf.extend_from_slice(b"\x1b[G");   // go to column 0
        } else if self.printed_lines > 1 {
            buf.extend_from_slice(b"\x1b[G");   // column 0
            for _ in 0..(self.printed_lines - 1) {
                buf.extend_from_slice(b"\x1b[F"); // cursor previous line
            }
        }
        // DON'T clear lines here — content will be overwritten below.
        // \x1b[K after each line cleans any trailing garbage.

        // ── Phase 2 + 3: Write all content (stream + beacon) ──
        // After erase, cursor sits on the first cleared line (column 0).
        // The FIRST line of content writes directly there (no \n).
        // All subsequent lines are preceded by \n.
        let drained: Vec<String> = self.stream_buffer.drain(..).collect();

        // need_newline: false for the very first line written this frame.
        // On the first frame ever (printed_lines == 0, no stream), we also
        // don't need \n because the cursor is already at the right place.
        let mut need_newline = false;

        for line in &drained {
            if need_newline {
                buf.extend_from_slice(b"\n");
            }
            buf.extend_from_slice(line.as_bytes());
            buf.extend_from_slice(b"\x1b[K"); // clear trailing garbage
            need_newline = true;
        }

        for line in beacon_lines.iter() {
            if need_newline {
                buf.extend_from_slice(b"\n");
            }
            buf.extend_from_slice(line.as_bytes());
            buf.extend_from_slice(b"\x1b[K"); // clear trailing garbage
            need_newline = true;
        }

        // ── Phase 4: If beacon shrank, clear leftover lines below ──
        let new_count = beacon_lines.len();
        let total_written = drained.len() + new_count;
        if self.printed_lines > total_written {
            let leftover = self.printed_lines - total_written;
            for _ in 0..leftover {
                buf.extend_from_slice(b"\n\x1b[2K");
            }
            // Move back up so cursor stays on last beacon line
            if leftover > 0 {
                buf.extend_from_slice(format!("\x1b[{}A", leftover).as_bytes());
            }
        }

        // ── End synchronized update ──
        buf.extend_from_slice(SYNC_END.as_bytes());

        // ── Single write syscall ──
        let _ = self.term.write_all(&buf);
        let _ = self.term.flush();

        self.printed_lines = new_count;
    }

    /// Clear beacon and print final static content.
    pub fn print_final(&mut self, lines: &[String]) {
        // Erase current beacon
        if self.printed_lines > 0 {
            let mut buf = Vec::<u8>::new();
            buf.extend_from_slice(b"\x1b[2K");
            for _ in 0..self.printed_lines.saturating_sub(1) {
                buf.extend_from_slice(b"\x1b[F\x1b[2K");
            }
            let _ = self.term.write_all(&buf);
            let _ = self.term.flush();
            self.printed_lines = 0;
        }
        // Print final content as normal scrollback
        for line in lines {
            let _ = self.term.write_line(line);
        }
        self.show_cursor();
    }

    pub fn hide_cursor(&mut self) {
        if !self.cursor_hidden {
            install_cursor_restore_hook();
            let _ = self.term.hide_cursor();
            self.cursor_hidden = true;
            CURSOR_HIDDEN.store(true, Ordering::SeqCst);
        }
    }

    pub fn show_cursor(&mut self) {
        if self.cursor_hidden {
            let _ = self.term.show_cursor();
            self.cursor_hidden = false;
            CURSOR_HIDDEN.store(false, Ordering::SeqCst);
        }
    }

    pub fn set_width(&mut self, width: u16) {
        self.term_width = width;
    }

    pub fn count_wrapped_lines(&self, line: &str) -> usize {
        if self.term_width == 0 { return 1; }
        let visible_len = console::measure_text_width(line);
        if visible_len == 0 { return 1; }
        (visible_len + self.term_width as usize - 1) / self.term_width as usize
    }
}

impl Drop for Painter {
    fn drop(&mut self) {
        self.show_cursor();
    }
}

fn install_cursor_restore_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = ctrlc::set_handler(move || {
            let _ = std::io::stderr().write_all(b"\x1b[?25h");
            let _ = std::io::stderr().flush();
            std::process::exit(130);
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_wrapped_lines_short() {
        let p = Painter::new(80);
        assert_eq!(p.count_wrapped_lines("hello"), 1);
    }

    #[test]
    fn count_wrapped_lines_long() {
        let p = Painter::new(10);
        assert_eq!(p.count_wrapped_lines("12345678901234567890"), 2);
    }

    #[test]
    fn count_wrapped_lines_empty() {
        let p = Painter::new(80);
        assert_eq!(p.count_wrapped_lines(""), 1);
    }
}
