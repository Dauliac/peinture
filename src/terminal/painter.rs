//! Painter — nom-style terminal renderer with pinned region at bottom.
//!
//! Each frame (single atomic write with sync update):
//! 1. Move cursor to start of previous beacon (`\x1b[nF`)
//! 2. Clear to end of screen (`\x1b[J`)
//! 3. Print stream content (scrolls into history)
//! 4. Print beacon (NO trailing newline on last line)
//!
//! Cursor ends at the end of the last beacon line.
//! Next frame moves up by `pinned_count - 1` lines.

use console::Term;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use super::sync_update::{SYNC_BEGIN, SYNC_END};

static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

pub struct Painter {
    term: Term,
    /// Lines the beacon occupied last frame.
    pinned_count: usize,
    /// Pending stream lines.
    stream_buffer: Vec<String>,
    cursor_hidden: bool,
    term_width: u16,
}

impl Painter {
    pub fn new(term_width: u16) -> Self {
        Self {
            term: Term::stderr(),
            pinned_count: 0,
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

    /// Render a frame (single atomic write).
    /// Render a frame — overwrite then clear trailing (nom-style).
    ///
    /// Never blanks a line before writing. Content overwrites old content
    /// in place, then `\x1b[K` clears only the trailing remainder.
    /// No visible flash even without synchronized update support.
    pub fn render_frame(&mut self, pinned_lines: &[String]) {
        let old_count = self.pinned_count;
        let new_count = pinned_lines.len();

        let mut buf = String::new();
        buf.push_str(SYNC_BEGIN);

        // 1. Move cursor to start of previous beacon
        if old_count > 1 {
            buf.push_str(&format!("\x1b[{}F", old_count - 1));
        } else if old_count == 1 {
            buf.push('\r');
        }

        // 2. Stream lines: overwrite + clear trailing + newline.
        //    The \n scrolls old beacon content up into scrollback.
        let drained: Vec<String> = self.stream_buffer.drain(..).collect();
        for line in &drained {
            buf.push_str(line);
            buf.push_str("\x1b[K\n"); // clear rest of line, then newline
        }

        // 3. Overwrite beacon lines — write content, THEN clear trailing.
        //    Old content is overwritten char-by-char, never blanked first.
        for (i, line) in pinned_lines.iter().enumerate() {
            buf.push('\r');          // go to column 0
            buf.push_str(line);      // overwrite old content
            buf.push_str("\x1b[K");  // clear anything remaining after
            if i < new_count - 1 {
                buf.push('\n');
            }
        }

        // 4. If beacon shrank, clear leftover lines below
        if new_count < old_count {
            for _ in 0..(old_count - new_count) {
                buf.push_str("\n\x1b[2K");
            }
            buf.push_str(&format!("\x1b[{}F", old_count - new_count));
        }

        buf.push_str(SYNC_END);

        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();

        self.pinned_count = new_count;
    }

    /// Clear beacon and print final static content.
    pub fn print_final(&mut self, lines: &[String]) {
        // Erase beacon
        if self.pinned_count > 1 {
            let mut buf = String::new();
            buf.push_str(&format!("\x1b[{}F", self.pinned_count - 1));
            buf.push_str("\x1b[J");
            let _ = self.term.write_all(buf.as_bytes());
            let _ = self.term.flush();
        } else if self.pinned_count == 1 {
            let _ = self.term.write_all(b"\r\x1b[J");
            let _ = self.term.flush();
        }
        self.pinned_count = 0;

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
