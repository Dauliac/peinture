//! Painter — scroll region renderer (the approach that worked without blink).
//!
//! The terminal is split into two zones:
//! - **Scroll region** (rows 1..scroll_bottom): stream content scrolls here
//! - **Beacon area** (rows scroll_bottom+1..term_height): redrawn with absolute positioning
//!
//! The beacon area is OUTSIDE the scroll region — it never scrolls,
//! never blinks. Each frame just overwrites each beacon line at its
//! absolute row position.
//!
//! When beacon height changes (notification added/removed):
//! - Resize the scroll region in the same synchronized frame
//! - Clear any freed rows
//! - No blink because everything is in one atomic write

use console::Term;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use super::sync_update::{SYNC_BEGIN, SYNC_END};

static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

pub struct Painter {
    term: Term,
    pinned_line_count: usize,
    stream_buffer: Vec<String>,
    cursor_hidden: bool,
    term_width: u16,
    term_height: u16,
    initialized: bool,
}

impl Painter {
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

    pub fn stream_line(&mut self, line: String) {
        self.stream_buffer.push(line);
    }

    pub fn stream_lines(&mut self, lines: impl IntoIterator<Item = String>) {
        self.stream_buffer.extend(lines);
    }

    /// Render a frame — single atomic write, no blink.
    pub fn render_frame(&mut self, pinned_lines: &[String]) {
        let new_height = pinned_lines.len() as u16;
        let old_height = self.pinned_line_count as u16;

        let mut buf = String::with_capacity(4096);
        buf.push_str(SYNC_BEGIN);

        // First frame: initialize scroll region
        if !self.initialized {
            // Push existing content up
            for _ in 0..self.term_height {
                buf.push('\n');
            }
            let scroll_bottom = self.term_height.saturating_sub(new_height);
            buf.push_str(&format!("\x1b[1;{scroll_bottom}r"));
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));
            self.initialized = true;
        } else if new_height != old_height {
            // Beacon height changed: resize scroll region in same frame.
            // First clear old beacon area (using full terminal, no scroll region)
            buf.push_str("\x1b[r"); // reset scroll region temporarily
            let old_start = self.term_height.saturating_sub(old_height) + 1;
            for row in old_start..=self.term_height {
                buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
            }
            // Set new scroll region
            let scroll_bottom = self.term_height.saturating_sub(new_height);
            buf.push_str(&format!("\x1b[1;{scroll_bottom}r"));
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));
        }

        let scroll_bottom = self.term_height.saturating_sub(new_height);
        let pinned_start = scroll_bottom + 1;

        // Stream content: print in scroll region (scrolls naturally)
        if !self.stream_buffer.is_empty() {
            let drained: Vec<String> = self.stream_buffer.drain(..).collect();
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));
            for line in &drained {
                buf.push_str(&format!("\n{line}\x1b[K"));
            }
        }

        // Beacon: overwrite at absolute positions (outside scroll region, never scrolls)
        for (i, line) in pinned_lines.iter().enumerate() {
            let row = pinned_start + i as u16;
            buf.push_str(&format!("\x1b[{row};1H{line}\x1b[K"));
        }

        // Park cursor in scroll region
        buf.push_str(&format!("\x1b[{scroll_bottom};1H"));

        buf.push_str(SYNC_END);

        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();

        self.pinned_line_count = pinned_lines.len();
    }

    pub fn clear_pinned(&mut self) {
        if self.pinned_line_count > 0 {
            let old_start = self.term_height.saturating_sub(self.pinned_line_count as u16) + 1;
            let mut buf = String::new();
            buf.push_str(SYNC_BEGIN);
            for row in old_start..=self.term_height {
                buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
            }
            buf.push_str(SYNC_END);
            let _ = self.term.write_all(buf.as_bytes());
            let _ = self.term.flush();
            self.pinned_line_count = 0;
        }
        let _ = self.term.write_all(b"\x1b[r");
        let _ = self.term.flush();
    }

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

    pub fn set_size(&mut self, width: u16, height: u16) {
        self.term_width = width;
        self.term_height = height;
        self.initialized = false;
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
        let _ = self.term.write_all(b"\x1b[r\x1b[?25h");
        let _ = self.term.flush();
        self.cursor_hidden = false;
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
