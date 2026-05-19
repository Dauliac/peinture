//! Painter — nom-style terminal renderer with pinned region at bottom.
//!
//! The painter manages two zones:
//! 1. **Stream zone** — output that scrolls into scrollback (build logs, etc.)
//! 2. **Pinned zone** — fixed-height region at the bottom (beacon)
//!
//! Each render frame:
//! 1. Erase the pinned region (cursor-up + clear-line for each pinned line)
//! 2. Print any new stream content (becomes part of scrollback)
//! 3. Redraw the pinned region
//!
//! All writes go to stderr (stdout reserved for machine-readable output).

use console::Term;
use std::io::Write;
use super::sync_update::{SYNC_BEGIN, SYNC_END};

/// Terminal painter that manages streaming output with a pinned region.
pub struct Painter {
    term: Term,
    /// Number of lines currently occupied by the pinned region.
    pinned_line_count: usize,
    /// Pending stream lines to flush before next pinned redraw.
    stream_buffer: Vec<String>,
    /// Whether the cursor is currently hidden.
    cursor_hidden: bool,
    /// Current terminal width (for reflow correction).
    term_width: u16,
}

impl Painter {
    /// Create a new painter writing to stderr.
    pub fn new(term_width: u16) -> Self {
        Self {
            term: Term::stderr(),
            pinned_line_count: 0,
            stream_buffer: Vec::new(),
            cursor_hidden: false,
            term_width,
        }
    }

    /// Queue a line of stream content (will be flushed before next pinned redraw).
    pub fn stream_line(&mut self, line: String) {
        self.stream_buffer.push(line);
    }

    /// Queue multiple stream lines.
    pub fn stream_lines(&mut self, lines: impl IntoIterator<Item = String>) {
        self.stream_buffer.extend(lines);
    }

    /// Render a frame: flush stream content, then redraw the pinned region.
    ///
    /// `pinned_lines` is the new content for the pinned region.
    /// Pass an empty slice to clear the pinned region.
    pub fn render_frame(&mut self, pinned_lines: &[String]) {
        let mut buf = String::new();

        // Begin synchronized update
        buf.push_str(SYNC_BEGIN);

        // 1. Erase current pinned region
        if self.pinned_line_count > 0 {
            // Move cursor up to the top of the pinned region
            buf.push_str(&format!("\x1b[{}A", self.pinned_line_count));
            // Clear each line
            for _ in 0..self.pinned_line_count {
                buf.push_str("\x1b[2K\n"); // clear line + move down
            }
            // Move back up to start of pinned region
            buf.push_str(&format!("\x1b[{}A", self.pinned_line_count));
        }

        // 2. Print stream content (becomes scrollback)
        let drained: Vec<String> = self.stream_buffer.drain(..).collect();
        for line in &drained {
            buf.push_str(line);
            buf.push('\n');
        }

        // 3. Draw new pinned region
        for line in pinned_lines {
            buf.push_str(line);
            buf.push('\n');
        }

        // End synchronized update
        buf.push_str(SYNC_END);

        // Write atomically
        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();

        self.pinned_line_count = pinned_lines.len();
    }

    /// Clear the pinned region entirely (e.g., on completion).
    pub fn clear_pinned(&mut self) {
        if self.pinned_line_count > 0 {
            let mut buf = String::new();
            buf.push_str(SYNC_BEGIN);
            buf.push_str(&format!("\x1b[{}A", self.pinned_line_count));
            for _ in 0..self.pinned_line_count {
                buf.push_str("\x1b[2K\n");
            }
            buf.push_str(&format!("\x1b[{}A", self.pinned_line_count));
            buf.push_str(SYNC_END);
            let _ = self.term.write_all(buf.as_bytes());
            let _ = self.term.flush();
            self.pinned_line_count = 0;
        }
    }

    /// Print final static content (no pinned tracking — becomes scrollback).
    pub fn print_final(&mut self, lines: &[String]) {
        self.clear_pinned();
        for line in lines {
            let _ = self.term.write_line(line);
        }
        self.show_cursor();
    }

    /// Hide the terminal cursor (call at start of animation).
    pub fn hide_cursor(&mut self) {
        if !self.cursor_hidden {
            let _ = self.term.hide_cursor();
            self.cursor_hidden = true;
        }
    }

    /// Show the terminal cursor (call on completion or signal).
    pub fn show_cursor(&mut self) {
        if self.cursor_hidden {
            let _ = self.term.show_cursor();
            self.cursor_hidden = false;
        }
    }

    /// Update terminal width (call on resize).
    pub fn set_term_width(&mut self, width: u16) {
        self.term_width = width;
    }

    /// Count how many terminal lines a string occupies (for reflow correction).
    fn count_wrapped_lines(&self, line: &str) -> usize {
        if self.term_width == 0 {
            return 1;
        }
        let visible_len = console::measure_text_width(line);
        if visible_len == 0 {
            return 1;
        }
        (visible_len + self.term_width as usize - 1) / self.term_width as usize
    }
}

impl Drop for Painter {
    fn drop(&mut self) {
        // Always restore cursor on cleanup
        self.show_cursor();
    }
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
