//! Painter — scroll region renderer with stream line buffer.
//!
//! Stream lines go into a FIFO buffer (size = max beacon height).
//! Each frame, overflow lines are flushed to the screen.
//! When a notification is removed (beacon shrinks by N), N extra
//! lines are pulled from the buffer to fill the freed space.
//! No blank lines visible during the transition.

use console::Term;
use std::collections::VecDeque;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use super::sync_update::{SYNC_BEGIN, SYNC_END};

static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

pub struct Painter {
    term: Term,
    pinned_line_count: usize,
    /// FIFO buffer for stream lines. Always keeps `reserve` lines ready.
    buffer: VecDeque<String>,
    /// How many lines to keep in reserve (= max beacon height).
    reserve: usize,
    cursor_hidden: bool,
    term_width: u16,
    term_height: u16,
    initialized: bool,
}

impl Painter {
    /// Create a painter. `reserve` is the stream buffer size
    /// (use `theme.beacon.max_items + 1`).
    pub fn new(term_width: u16, term_height: u16, reserve: usize) -> Self {
        Self {
            term: Term::stderr(),
            pinned_line_count: 0,
            buffer: VecDeque::new(),
            reserve,
            cursor_hidden: false,
            term_width,
            term_height,
            initialized: false,
        }
    }

    /// Push a stream line into the buffer.
    pub fn stream_line(&mut self, line: String) {
        self.buffer.push_back(line);
    }

    /// Push multiple stream lines.
    pub fn stream_lines(&mut self, lines: impl IntoIterator<Item = String>) {
        self.buffer.extend(lines);
    }

    /// Render a frame.
    pub fn render_frame(&mut self, pinned_lines: &[String]) {
        let new_height = pinned_lines.len() as u16;
        let old_height = self.pinned_line_count as u16;

        // How many lines to flush from buffer this frame:
        // - overflow: lines beyond the reserve
        // - gap fill: if beacon shrank, pull extra to fill freed rows
        let gap = if new_height < old_height {
            (old_height - new_height) as usize
        } else {
            0
        };
        let overflow = self.buffer.len().saturating_sub(self.reserve);
        let to_flush = overflow + gap;
        let to_flush = to_flush.min(self.buffer.len()); // can't flush more than we have

        // Drain lines to flush from front of buffer
        let flushed: Vec<String> = self.buffer.drain(..to_flush).collect();

        let mut buf = String::with_capacity(4096);
        buf.push_str(SYNC_BEGIN);

        // First frame: initialize
        if !self.initialized {
            for _ in 0..self.term_height {
                buf.push('\n');
            }
            let scroll_bottom = self.term_height.saturating_sub(new_height);
            buf.push_str(&format!("\x1b[1;{scroll_bottom}r"));
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));
            self.initialized = true;
        } else if new_height != old_height {
            // Beacon height changed — resize scroll region
            buf.push_str("\x1b[r"); // reset
            let old_start = self.term_height.saturating_sub(old_height) + 1;
            for row in old_start..=self.term_height {
                buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
            }
            let scroll_bottom = self.term_height.saturating_sub(new_height);
            buf.push_str(&format!("\x1b[1;{scroll_bottom}r"));
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));
        }

        let scroll_bottom = self.term_height.saturating_sub(new_height);
        let pinned_start = scroll_bottom + 1;

        // Stream: flush lines into scroll region
        if !flushed.is_empty() {
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));
            for line in &flushed {
                buf.push_str(&format!("\n{line}\x1b[K"));
            }
        }

        // Beacon: absolute positioning
        for (i, line) in pinned_lines.iter().enumerate() {
            let row = pinned_start + i as u16;
            buf.push_str(&format!("\x1b[{row};1H{line}\x1b[K"));
        }

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
        // Flush remaining buffer
        if !self.buffer.is_empty() {
            let scroll_bottom = self.term_height.saturating_sub(self.pinned_line_count as u16);
            let mut buf = String::new();
            buf.push_str(SYNC_BEGIN);
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));
            for line in self.buffer.drain(..) {
                buf.push_str(&format!("\n{line}\x1b[K"));
            }
            buf.push_str(SYNC_END);
            let _ = self.term.write_all(buf.as_bytes());
            let _ = self.term.flush();
        }
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

    #[test]
    fn buffer_reserves_lines() {
        let mut p = Painter::new(80, 24, 3);
        p.stream_line("a".into());
        p.stream_line("b".into());
        p.stream_line("c".into());
        // 3 lines in buffer, reserve=3 → overflow=0, nothing flushed
        assert_eq!(p.buffer.len(), 3);

        p.stream_line("d".into());
        // 4 lines in buffer, reserve=3 → overflow=1
        assert_eq!(p.buffer.len(), 4);
    }
}
