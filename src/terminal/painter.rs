//! Painter — terminal renderer with a pinned region stuck to the bottom.
//!
//! Uses **scroll regions** to keep the beacon fixed at the bottom:
//! - Scroll region: row 1 to `term_height - beacon_height` (stream zone)
//! - Fixed region: last N rows (beacon, outside scroll region)
//!
//! Stream content scrolls normally within the scroll region.
//! The beacon is redrawn in-place each frame using absolute cursor positioning.
//!
//! All writes go to stderr (stdout reserved for machine-readable output).

use console::Term;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use super::sync_update::{SYNC_BEGIN, SYNC_END};

/// Global flag: set to true when cursor is hidden.
static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

/// Terminal painter that manages streaming output with a pinned region
/// stuck to the bottom of the terminal.
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
    /// Current terminal height.
    term_height: u16,
    /// Whether the scroll region has been set up.
    initialized: bool,
}

impl Painter {
    /// Create a new painter writing to stderr.
    pub fn new(term_width: u16, term_height: u16) -> Self {
        Self {
            term: Term::stderr(),
            pinned_line_count: 0,
            stream_buffer: Vec::new(),
            cursor_hidden: false,
            term_width,
            term_height,
            initialized: false,
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

    /// Render a frame: flush stream content in scroll region, redraw beacon below.
    pub fn render_frame(&mut self, pinned_lines: &[String]) {
        let beacon_height = pinned_lines.len() as u16;

        // Set up scroll region on first frame (or if beacon height changed)
        if !self.initialized || beacon_height as usize != self.pinned_line_count {
            self.setup_scroll_region(beacon_height);
        }

        let scroll_bottom = self.term_height.saturating_sub(beacon_height);
        let pinned_start = scroll_bottom + 1;

        let mut buf = String::new();
        buf.push_str(SYNC_BEGIN);

        // 1. Print stream content within the scroll region
        if !self.stream_buffer.is_empty() {
            let drained: Vec<String> = self.stream_buffer.drain(..).collect();

            // Move cursor to the last row of the scroll region
            // Printing here with \n scrolls content up within the region
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));

            for line in &drained {
                // Newline scrolls the region up, then write the line
                buf.push_str(&format!("\n\x1b[2K{line}"));
            }
        }

        // 2. Draw pinned region at the absolute bottom rows (outside scroll region)
        for (i, line) in pinned_lines.iter().enumerate() {
            let row = pinned_start + i as u16;
            buf.push_str(&format!("\x1b[{row};1H\x1b[2K{line}"));
        }

        // Park cursor at the bottom of scroll region (just above beacon)
        buf.push_str(&format!("\x1b[{scroll_bottom};1H"));

        buf.push_str(SYNC_END);

        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();

        self.pinned_line_count = pinned_lines.len();
    }

    /// Set up the scroll region: rows 1..scroll_bottom scroll,
    /// rows below are fixed for the beacon.
    fn setup_scroll_region(&mut self, beacon_height: u16) {
        let scroll_bottom = self.term_height.saturating_sub(beacon_height);

        let mut buf = String::new();

        if !self.initialized {
            // First time: push existing content up so beacon area is clear
            // Print enough newlines to ensure cursor is at the bottom
            for _ in 0..self.term_height {
                buf.push('\n');
            }
        }

        // Set scroll region: row 1 to scroll_bottom
        buf.push_str(&format!("\x1b[1;{scroll_bottom}r"));

        // Move cursor to the bottom of the scroll region
        buf.push_str(&format!("\x1b[{scroll_bottom};1H"));

        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();

        self.initialized = true;
    }

    /// Clear the pinned region and reset scroll region.
    pub fn clear_pinned(&mut self) {
        if self.pinned_line_count > 0 {
            let beacon_height = self.pinned_line_count as u16;
            let pinned_start = self.term_height.saturating_sub(beacon_height) + 1;

            let mut buf = String::new();
            buf.push_str(SYNC_BEGIN);
            for row in pinned_start..=self.term_height {
                buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
            }
            buf.push_str(SYNC_END);

            let _ = self.term.write_all(buf.as_bytes());
            let _ = self.term.flush();
            self.pinned_line_count = 0;
        }

        // Reset scroll region to full terminal
        let _ = self.term.write_all(b"\x1b[r");
        let _ = self.term.flush();
    }

    /// Print final static content (no pinned tracking — becomes scrollback).
    pub fn print_final(&mut self, lines: &[String]) {
        self.clear_pinned();
        // Move to the bottom of the terminal
        let _ = self.term.write_all(format!("\x1b[{};1H", self.term_height).as_bytes());
        let _ = self.term.flush();
        for line in lines {
            let _ = self.term.write_line(line);
        }
        self.show_cursor();
        self.initialized = false;
    }

    /// Hide the terminal cursor (call at start of animation).
    pub fn hide_cursor(&mut self) {
        if !self.cursor_hidden {
            install_cursor_restore_hook();
            let _ = self.term.hide_cursor();
            self.cursor_hidden = true;
            CURSOR_HIDDEN.store(true, Ordering::SeqCst);
        }
    }

    /// Show the terminal cursor (call on completion or signal).
    pub fn show_cursor(&mut self) {
        if self.cursor_hidden {
            let _ = self.term.show_cursor();
            self.cursor_hidden = false;
            CURSOR_HIDDEN.store(false, Ordering::SeqCst);
        }
    }

    /// Update terminal dimensions (call on resize / SIGWINCH).
    pub fn set_size(&mut self, width: u16, height: u16) {
        self.term_width = width;
        self.term_height = height;
        self.initialized = false; // force scroll region recalculation
    }

    /// Count how many terminal lines a string occupies (for reflow correction).
    pub fn count_wrapped_lines(&self, line: &str) -> usize {
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
        // Reset scroll region and restore cursor
        let _ = self.term.write_all(b"\x1b[r");
        let _ = self.term.flush();
        self.show_cursor();
    }
}

/// Install a ctrlc handler that restores cursor and scroll region before exiting.
fn install_cursor_restore_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let _ = ctrlc::set_handler(move || {
            // Reset scroll region + show cursor
            let _ = std::io::stderr().write_all(b"\x1b[r\x1b[?25h");
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
        let p = Painter::new(80, 24);
        assert_eq!(p.count_wrapped_lines("hello"), 1);
    }

    #[test]
    fn count_wrapped_lines_long() {
        let p = Painter::new(10, 24);
        assert_eq!(p.count_wrapped_lines("12345678901234567890"), 2);
    }

    #[test]
    fn count_wrapped_lines_empty() {
        let p = Painter::new(80, 24);
        assert_eq!(p.count_wrapped_lines(""), 1);
    }
}
