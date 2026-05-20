//! Painter — terminal renderer with a fixed pinned region at the bottom.
//!
//! Uses **scroll regions** to keep the beacon fixed:
//! - Scroll region: row 1 to `term_height - reserved_height` (stream zone)
//! - Reserved region: last N rows (beacon area, outside scroll region)
//!
//! The reserved height is set once via `set_reserved_height()` and never
//! changes. Content is drawn at the BOTTOM of the reserved area — empty
//! rows above are cleared, not filled with blank lines.

use console::Term;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use super::sync_update::{SYNC_BEGIN, SYNC_END};

static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

/// Terminal painter with a fixed pinned region at the bottom.
pub struct Painter {
    term: Term,
    /// Pending stream lines to flush.
    stream_buffer: Vec<String>,
    /// Whether the cursor is currently hidden.
    cursor_hidden: bool,
    /// Current terminal width.
    term_width: u16,
    /// Current terminal height.
    term_height: u16,
    /// Fixed number of rows reserved for the pinned region.
    /// Set once, never changes — prevents scroll region resize artifacts.
    reserved_height: u16,
    /// Whether the scroll region has been set up.
    initialized: bool,
}

impl Painter {
    /// Create a new painter.
    ///
    /// `reserved_height` is the fixed number of rows for the beacon area.
    /// Use `theme.beacon.max_items + 1` for the beacon.
    pub fn new(term_width: u16, term_height: u16, reserved_height: u16) -> Self {
        Self {
            term: Term::stderr(),
            stream_buffer: Vec::new(),
            cursor_hidden: false,
            term_width,
            term_height,
            reserved_height,
            initialized: false,
        }
    }

    /// Queue a line of stream content.
    pub fn stream_line(&mut self, line: String) {
        self.stream_buffer.push(line);
    }

    /// Queue multiple stream lines.
    pub fn stream_lines(&mut self, lines: impl IntoIterator<Item = String>) {
        self.stream_buffer.extend(lines);
    }

    /// Render a frame: flush stream content, redraw beacon at bottom of reserved area.
    ///
    /// `pinned_lines` is the actual beacon content (may be shorter than reserved_height).
    /// Content is drawn at the BOTTOM of the reserved area. Empty rows above are cleared.
    pub fn render_frame(&mut self, pinned_lines: &[String]) {
        if !self.initialized {
            self.setup_scroll_region();
        }

        let scroll_bottom = self.term_height.saturating_sub(self.reserved_height);
        let reserved_start = scroll_bottom + 1;
        let content_height = pinned_lines.len() as u16;

        // Content starts at bottom of reserved area, not top
        let content_start = self.term_height.saturating_sub(content_height) + 1;

        let mut buf = String::new();
        buf.push_str(SYNC_BEGIN);

        // 1. Print stream content within the scroll region
        if !self.stream_buffer.is_empty() {
            let drained: Vec<String> = self.stream_buffer.drain(..).collect();
            for line in &drained {
                buf.push_str(&format!("\x1b[{scroll_bottom};1H\n\x1b[2K{line}"));
            }
        }

        // 2. Clear ALL reserved rows, then draw content at the bottom
        for row in reserved_start..=self.term_height {
            buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
        }
        for (i, line) in pinned_lines.iter().enumerate() {
            let row = content_start + i as u16;
            buf.push_str(&format!("\x1b[{row};1H{line}"));
        }

        // Park cursor at bottom of scroll region
        buf.push_str(&format!("\x1b[{scroll_bottom};1H"));

        buf.push_str(SYNC_END);

        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();
    }

    /// Set up the scroll region (once).
    fn setup_scroll_region(&mut self) {
        let scroll_bottom = self.term_height.saturating_sub(self.reserved_height);

        let mut buf = String::new();
        // Push content off screen so reserved area is clean
        for _ in 0..self.term_height {
            buf.push('\n');
        }
        // Set scroll region
        buf.push_str(&format!("\x1b[1;{scroll_bottom}r"));
        buf.push_str(&format!("\x1b[{scroll_bottom};1H"));

        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();
        self.initialized = true;
    }

    /// Clear pinned region and reset scroll region.
    pub fn clear_pinned(&mut self) {
        let reserved_start = self.term_height.saturating_sub(self.reserved_height) + 1;
        let mut buf = String::new();
        buf.push_str(SYNC_BEGIN);
        for row in reserved_start..=self.term_height {
            buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
        }
        buf.push_str(SYNC_END);
        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();

        // Reset scroll region
        let _ = self.term.write_all(b"\x1b[r");
        let _ = self.term.flush();
    }

    /// Print final static content as scrollback.
    pub fn print_final(&mut self, lines: &[String]) {
        self.clear_pinned();
        let _ = self.term.write_all(format!("\x1b[{};1H", self.term_height).as_bytes());
        let _ = self.term.flush();
        for line in lines {
            let _ = self.term.write_line(line);
        }
        self.show_cursor();
        self.initialized = false;
    }

    /// Hide the terminal cursor.
    pub fn hide_cursor(&mut self) {
        if !self.cursor_hidden {
            install_cursor_restore_hook();
            let _ = self.term.hide_cursor();
            self.cursor_hidden = true;
            CURSOR_HIDDEN.store(true, Ordering::SeqCst);
        }
    }

    /// Show the terminal cursor.
    pub fn show_cursor(&mut self) {
        if self.cursor_hidden {
            let _ = self.term.show_cursor();
            self.cursor_hidden = false;
            CURSOR_HIDDEN.store(false, Ordering::SeqCst);
        }
    }

    /// Update terminal dimensions.
    pub fn set_size(&mut self, width: u16, height: u16) {
        self.term_width = width;
        self.term_height = height;
        self.initialized = false;
    }

    /// Count wrapped lines for a string.
    pub fn count_wrapped_lines(&self, line: &str) -> usize {
        if self.term_width == 0 { return 1; }
        let visible_len = console::measure_text_width(line);
        if visible_len == 0 { return 1; }
        (visible_len + self.term_width as usize - 1) / self.term_width as usize
    }
}

impl Drop for Painter {
    fn drop(&mut self) {
        let _ = self.term.write_all(b"\x1b[r");
        let _ = self.term.flush();
        self.show_cursor();
    }
}

fn install_cursor_restore_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        let _ = ctrlc::set_handler(move || {
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
        let p = Painter::new(80, 24, 6);
        assert_eq!(p.count_wrapped_lines("hello"), 1);
    }

    #[test]
    fn count_wrapped_lines_long() {
        let p = Painter::new(10, 24, 6);
        assert_eq!(p.count_wrapped_lines("12345678901234567890"), 2);
    }

    #[test]
    fn count_wrapped_lines_empty() {
        let p = Painter::new(80, 24, 6);
        assert_eq!(p.count_wrapped_lines(""), 1);
    }
}
