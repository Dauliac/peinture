//! Painter — nom-style terminal renderer with pinned region at bottom.
//!
//! Each frame:
//! 1. Cursor up by last beacon height
//! 2. Clear those lines
//! 3. Print stream content (becomes scrollback)
//! 4. Print beacon content
//!
//! No scroll regions, no fixed reservations. The beacon is simply
//! the last N lines on screen, redrawn each frame.

use console::Term;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use super::sync_update::{SYNC_BEGIN, SYNC_END};

static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

/// Terminal painter — nom-style pinned region at bottom.
pub struct Painter {
    term: Term,
    /// How many lines the beacon occupied last frame.
    pinned_line_count: usize,
    /// Pending stream lines to flush.
    stream_buffer: Vec<String>,
    cursor_hidden: bool,
    term_width: u16,
}

impl Painter {
    pub fn new(term_width: u16) -> Self {
        Self {
            term: Term::stderr(),
            pinned_line_count: 0,
            stream_buffer: Vec::new(),
            cursor_hidden: false,
            term_width,
        }
    }

    /// Queue a stream line (will be printed above beacon next frame).
    pub fn stream_line(&mut self, line: String) {
        self.stream_buffer.push(line);
    }

    /// Queue multiple stream lines.
    pub fn stream_lines(&mut self, lines: impl IntoIterator<Item = String>) {
        self.stream_buffer.extend(lines);
    }

    /// Render a frame.
    ///
    /// 1. Erase the previous beacon (cursor-up + clear)
    /// 2. Print any pending stream content (scrolls into history)
    /// 3. Draw the new beacon
    pub fn render_frame(&mut self, pinned_lines: &[String]) {
        let mut buf = String::new();
        buf.push_str(SYNC_BEGIN);

        // 1. Erase previous beacon
        if self.pinned_line_count > 0 {
            // Move to start of previous beacon
            buf.push_str(&format!("\x1b[{}A", self.pinned_line_count));
            for _ in 0..self.pinned_line_count {
                buf.push_str("\x1b[2K\n");
            }
            // Move back to where beacon started
            buf.push_str(&format!("\x1b[{}A", self.pinned_line_count));
        }

        // 2. Print stream content (becomes scrollback above beacon)
        let drained: Vec<String> = self.stream_buffer.drain(..).collect();
        for line in &drained {
            buf.push_str(line);
            buf.push_str("\x1b[0K\n"); // clear rest of line + newline
        }

        // 3. Draw new beacon
        for (i, line) in pinned_lines.iter().enumerate() {
            buf.push_str("\x1b[2K"); // clear line
            buf.push_str(line);
            if i < pinned_lines.len() - 1 {
                buf.push('\n');
            }
        }

        buf.push_str(SYNC_END);

        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();

        self.pinned_line_count = pinned_lines.len();
    }

    /// Clear beacon and print final static content.
    pub fn print_final(&mut self, lines: &[String]) {
        // Erase beacon
        if self.pinned_line_count > 0 {
            let mut buf = String::new();
            buf.push_str(&format!("\x1b[{}A", self.pinned_line_count));
            for _ in 0..self.pinned_line_count {
                buf.push_str("\x1b[2K\n");
            }
            buf.push_str(&format!("\x1b[{}A", self.pinned_line_count));
            let _ = self.term.write_all(buf.as_bytes());
            let _ = self.term.flush();
            self.pinned_line_count = 0;
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
