//! Painter — scroll region renderer with stream line buffer.
//!
//! Stream lines go into a FIFO buffer (size = max beacon height).
//! Each frame, overflow lines are flushed to the screen.
//! When a notification is removed (beacon shrinks by N), N extra
//! lines are pulled from the buffer to fill the freed space.
//!
//! Resize is handled separately from first-init:
//! - First-init: push minimal newlines, set scroll region
//! - Resize: clear old beacon at OLD position, set new scroll region

use console::Term;
use std::collections::VecDeque;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use super::sync_update::{SYNC_BEGIN, SYNC_END};

static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

pub struct Painter {
    term: Term,
    pinned_line_count: usize,
    buffer: VecDeque<String>,
    reserve: usize,
    cursor_hidden: bool,
    term_width: u16,
    term_height: u16,
    /// Previous terminal height — used to clear old beacon on resize.
    prev_term_height: u16,
    initialized: bool,
}

impl Painter {
    pub fn new(term_width: u16, term_height: u16, reserve: usize) -> Self {
        Self {
            term: Term::stderr(),
            pinned_line_count: 0,
            buffer: VecDeque::new(),
            reserve,
            cursor_hidden: false,
            term_width,
            term_height,
            prev_term_height: term_height,
            initialized: false,
        }
    }

    pub fn stream_line(&mut self, line: String) {
        self.buffer.push_back(line);
    }

    pub fn stream_lines(&mut self, lines: impl IntoIterator<Item = String>) {
        self.buffer.extend(lines);
    }

    pub fn render_frame(&mut self, pinned_lines: &[String]) {
        // ── Detect resize (separate from first-init) ──
        let (new_term_h, new_term_w) = self.term.size();
        let resized = self.initialized
            && (new_term_w != self.term_width || new_term_h != self.term_height);

        if resized {
            self.prev_term_height = self.term_height;
            self.term_width = new_term_w;
            self.term_height = new_term_h;
        }

        // Truncate all lines to terminal width
        let tw = self.term_width;
        let pinned_lines: Vec<String> = pinned_lines.iter()
            .map(|l| truncate_line(l, tw))
            .collect();

        let new_height = pinned_lines.len() as u16;
        let old_height = self.pinned_line_count as u16;

        // Buffer flush logic
        let gap = if !resized && new_height < old_height {
            (old_height - new_height) as usize
        } else {
            0
        };
        let overflow = self.buffer.len().saturating_sub(self.reserve);
        let to_flush = (overflow + gap).min(self.buffer.len());

        let tw = self.term_width;
        let flushed: Vec<String> = self.buffer.drain(..to_flush)
            .map(|l| truncate_line(&l, tw))
            .collect();

        let mut buf = String::with_capacity(4096);
        buf.push_str(SYNC_BEGIN);

        // ── Path 1: First-init (only on first frame ever) ──
        if !self.initialized {
            // Push only enough newlines for beacon space (not full screen)
            for _ in 0..new_height {
                buf.push('\n');
            }
            let scroll_bottom = self.term_height.saturating_sub(new_height);
            buf.push_str(&format!("\x1b[1;{scroll_bottom}r"));
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));
            self.initialized = true;

        // ── Path 2: Resize (clear old beacon, set new scroll region) ──
        } else if resized {
            // Reset scroll region to access all rows
            buf.push_str("\x1b[r");

            // Clear old beacon at its OLD position
            let old_beacon_start = self.prev_term_height.saturating_sub(old_height) + 1;
            for row in old_beacon_start..=self.prev_term_height {
                buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
            }

            // If terminal grew: clear new rows that might have artifacts
            if self.term_height > self.prev_term_height {
                for row in (self.prev_term_height + 1)..=self.term_height {
                    buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
                }
            }

            // Set new scroll region
            let scroll_bottom = self.term_height.saturating_sub(new_height);
            buf.push_str(&format!("\x1b[1;{scroll_bottom}r"));
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));

        // ── Path 3: Beacon height changed (notification added/removed) ──
        } else if new_height != old_height {
            buf.push_str("\x1b[r");

            let old_start = self.term_height.saturating_sub(old_height) + 1;
            let new_start = self.term_height.saturating_sub(new_height) + 1;
            let mut row = old_start;

            // Freed rows: fill with buffer lines
            let gap_lines = &flushed[..gap.min(flushed.len())];
            for line in gap_lines {
                buf.push_str(&format!("\x1b[{row};1H{line}\x1b[K"));
                row += 1;
            }
            while row < new_start {
                buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
                row += 1;
            }

            // Beacon rows: overwrite
            for (i, line) in pinned_lines.iter().enumerate() {
                let r = new_start + i as u16;
                buf.push_str(&format!("\x1b[{r};1H{line}\x1b[K"));
            }

            // New scroll region
            let scroll_bottom = self.term_height.saturating_sub(new_height);
            buf.push_str(&format!("\x1b[1;{scroll_bottom}r"));
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));

            // Remaining flushed lines
            let remaining = &flushed[gap.min(flushed.len())..];
            if !remaining.is_empty() {
                for line in remaining {
                    buf.push_str(&format!("\n{line}\x1b[K"));
                }
            }

            buf.push_str(&format!(
                "\x1b[{};1H",
                self.term_height.saturating_sub(new_height)
            ));
            buf.push_str(SYNC_END);

            let _ = self.term.write_all(buf.as_bytes());
            let _ = self.term.flush();
            self.pinned_line_count = pinned_lines.len();
            return;
        }

        // ── Normal frame + resize fallthrough: draw stream + beacon ──
        let scroll_bottom = self.term_height.saturating_sub(new_height);
        let pinned_start = scroll_bottom + 1;

        if !flushed.is_empty() {
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));
            for line in &flushed {
                buf.push_str(&format!("\n{line}\x1b[K"));
            }
        }

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
        self.prev_term_height = self.term_height;
        self.term_width = width;
        self.term_height = height;
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

/// Truncate a line to fit terminal width (respects ANSI escape codes).
fn truncate_line(line: &str, max_width: u16) -> String {
    let visible_width = console::measure_text_width(line);
    if visible_width <= max_width as usize {
        return line.to_string();
    }
    let mut result = String::new();
    let mut width = 0;
    let mut in_escape = false;
    for ch in line.chars() {
        if ch == '\x1b' {
            in_escape = true;
            result.push(ch);
        } else if in_escape {
            result.push(ch);
            if ch == 'm' { in_escape = false; }
        } else {
            let cw = console::measure_text_width(&ch.to_string());
            if width + cw > max_width as usize { break; }
            result.push(ch);
            width += cw;
        }
    }
    result.push_str("\x1b[0m");
    result
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
        assert_eq!(p.buffer.len(), 3);
        p.stream_line("d".into());
        assert_eq!(p.buffer.len(), 4);
    }
}
