//! Painter — nom-style terminal renderer with pinned region at bottom.
//!
//! Uses save/restore cursor position to avoid line-counting errors:
//! 1. Restore cursor to where beacon starts
//! 2. Clear from cursor to end of screen
//! 3. Print stream content (scrolls into history)
//! 4. Save cursor position (new beacon start)
//! 5. Draw beacon

use console::Term;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use super::sync_update::{SYNC_BEGIN, SYNC_END};

static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

/// Save cursor position.
const CURSOR_SAVE: &str = "\x1b7";
/// Restore cursor position.
const CURSOR_RESTORE: &str = "\x1b8";
/// Clear from cursor to end of screen.
const CLEAR_TO_END: &str = "\x1b[J";

pub struct Painter {
    term: Term,
    /// Whether we've saved the initial cursor position.
    has_saved_position: bool,
    /// Pending stream lines.
    stream_buffer: Vec<String>,
    cursor_hidden: bool,
    term_width: u16,
}

impl Painter {
    pub fn new(term_width: u16) -> Self {
        Self {
            term: Term::stderr(),
            has_saved_position: false,
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

    /// Render a frame.
    pub fn render_frame(&mut self, pinned_lines: &[String]) {
        let mut buf = String::new();
        buf.push_str(SYNC_BEGIN);

        if self.has_saved_position {
            // Restore to where beacon started last frame
            buf.push_str(CURSOR_RESTORE);
            // Clear everything from there to end of screen
            buf.push_str(CLEAR_TO_END);
        }

        // Print stream content (becomes scrollback)
        let drained: Vec<String> = self.stream_buffer.drain(..).collect();
        for line in &drained {
            buf.push_str(line);
            buf.push('\n');
        }

        // Save position — this is where the beacon starts
        buf.push_str(CURSOR_SAVE);

        // Draw beacon
        for (i, line) in pinned_lines.iter().enumerate() {
            buf.push_str(line);
            if i < pinned_lines.len() - 1 {
                buf.push('\n');
            }
        }

        buf.push_str(SYNC_END);

        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();

        self.has_saved_position = true;
    }

    /// Clear beacon and print final static content.
    pub fn print_final(&mut self, lines: &[String]) {
        if self.has_saved_position {
            let mut buf = String::new();
            buf.push_str(CURSOR_RESTORE);
            buf.push_str(CLEAR_TO_END);
            let _ = self.term.write_all(buf.as_bytes());
            let _ = self.term.flush();
            self.has_saved_position = false;
        }
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
