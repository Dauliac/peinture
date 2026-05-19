//! Painter — terminal renderer with a pinned region stuck to the bottom.
//!
//! The painter manages two zones:
//! 1. **Stream zone** — output that scrolls into scrollback (build logs, etc.)
//! 2. **Pinned zone** — fixed-height region anchored to the terminal bottom (beacon)
//!
//! On the first frame the painter pushes the cursor down with blank lines
//! so the pinned region occupies the last N rows of the visible terminal.
//! Subsequent frames use absolute cursor positioning (`\x1b[row;1H`) to
//! redraw the pinned region in-place. Stream content is printed just above
//! the pinned region and scrolls up naturally.
//!
//! All writes go to stderr (stdout reserved for machine-readable output).

use console::Term;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use super::sync_update::{SYNC_BEGIN, SYNC_END};

/// Global flag: set to true when cursor is hidden.
/// The ctrlc handler reads this to know whether to restore cursor.
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
    /// Whether the initial layout (push-to-bottom) has been done.
    initialized: bool,
    /// Row where stream content is written (1-based, just above pinned region).
    stream_row: u16,
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
            stream_row: 1,
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

    /// Render a frame: flush stream content above, then redraw beacon at bottom.
    ///
    /// `pinned_lines` is the new content for the pinned region.
    /// Pass an empty slice to clear the pinned region.
    pub fn render_frame(&mut self, pinned_lines: &[String]) {
        let beacon_height = pinned_lines.len() as u16;

        // First frame: push cursor to bottom so beacon occupies last rows
        if !self.initialized {
            self.initialize_layout(beacon_height);
        }

        let pinned_start_row = self.term_height.saturating_sub(beacon_height) + 1;
        self.stream_row = pinned_start_row.saturating_sub(1);

        let mut buf = String::new();
        buf.push_str(SYNC_BEGIN);

        // 1. Print stream content above the beacon
        if !self.stream_buffer.is_empty() {
            let drained: Vec<String> = self.stream_buffer.drain(..).collect();

            // Move cursor to the stream row (just above beacon)
            // First, scroll up to make room: erase beacon, print stream lines,
            // then redraw beacon below.
            // Strategy: move to pinned region start, scroll the content up
            // by inserting lines above the beacon.

            // Move to first row of pinned region
            buf.push_str(&format!("\x1b[{pinned_start_row};1H"));

            // Insert N blank lines at this position, pushing beacon down
            // (and off screen — we'll redraw it)
            for line in &drained {
                // Insert line: push everything below down by one
                buf.push_str("\x1b[L"); // insert line (shifts content below down)
                buf.push_str(&format!("\x1b[2K{line}")); // clear + write content
                buf.push_str(&format!("\x1b[{pinned_start_row};1H")); // stay at same row
            }
        }

        // 2. Draw pinned region at the absolute bottom rows
        let pinned_start = self.term_height.saturating_sub(beacon_height) + 1;
        for (i, line) in pinned_lines.iter().enumerate() {
            let row = pinned_start + i as u16;
            // Move to row, clear it, write beacon line
            buf.push_str(&format!("\x1b[{row};1H\x1b[2K{line}"));
        }

        // Clear any leftover lines if beacon shrank
        if beacon_height < self.pinned_line_count as u16 {
            let old_start = self.term_height.saturating_sub(self.pinned_line_count as u16) + 1;
            for row in old_start..pinned_start {
                buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
            }
        }

        // Park cursor just above the beacon (where stream content would appear)
        let park_row = pinned_start.saturating_sub(1).max(1);
        buf.push_str(&format!("\x1b[{park_row};1H"));

        buf.push_str(SYNC_END);

        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();

        self.pinned_line_count = pinned_lines.len();
    }

    /// Set up the initial terminal layout: push content down so
    /// the beacon occupies the last N rows of the visible area.
    fn initialize_layout(&mut self, beacon_height: u16) {
        let mut buf = String::new();

        // Print enough newlines to scroll the terminal so the cursor
        // ends up at the bottom. This ensures the beacon will be
        // drawn in the last rows of the visible area.
        let blank_lines = self.term_height.saturating_sub(1);
        for _ in 0..blank_lines {
            buf.push('\n');
        }

        // Now position cursor at first row of beacon
        let beacon_start = self.term_height.saturating_sub(beacon_height) + 1;
        buf.push_str(&format!("\x1b[{beacon_start};1H"));

        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();

        self.initialized = true;
    }

    /// Clear the pinned region entirely (e.g., on completion).
    pub fn clear_pinned(&mut self) {
        if self.pinned_line_count > 0 {
            let beacon_height = self.pinned_line_count as u16;
            let pinned_start = self.term_height.saturating_sub(beacon_height) + 1;

            let mut buf = String::new();
            buf.push_str(SYNC_BEGIN);
            for row in pinned_start..=self.term_height {
                buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
            }
            // Park cursor at the top of where the beacon was
            buf.push_str(&format!("\x1b[{pinned_start};1H"));
            buf.push_str(SYNC_END);

            let _ = self.term.write_all(buf.as_bytes());
            let _ = self.term.flush();
            self.pinned_line_count = 0;
        }
    }

    /// Print final static content (no pinned tracking — becomes scrollback).
    pub fn print_final(&mut self, lines: &[String]) {
        self.clear_pinned();
        // Reset to normal terminal mode: cursor at bottom, content scrolls normally
        let _ = self.term.write_all(format!("\x1b[{};1H", self.term_height).as_bytes());
        let _ = self.term.flush();
        // Print each line as normal scrollback
        for line in lines {
            let _ = self.term.write_line(line);
        }
        self.show_cursor();
        self.initialized = false;
    }

    /// Hide the terminal cursor (call at start of animation).
    ///
    /// Installs a signal handler (ctrlc) that restores the cursor
    /// if the process is killed. Safe to call multiple times.
    pub fn hide_cursor(&mut self) {
        if !self.cursor_hidden {
            // Install ctrlc handler once to restore cursor on kill
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
        // Force re-layout on next frame
        self.initialized = false;
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
        self.show_cursor();
    }
}

/// Install a ctrlc handler that restores the cursor before exiting.
/// Safe to call multiple times — only installs once.
fn install_cursor_restore_hook() {
    use std::sync::Once;
    static ONCE: Once = Once::new();

    ONCE.call_once(|| {
        let _ = ctrlc::set_handler(move || {
            if CURSOR_HIDDEN.load(Ordering::SeqCst) {
                // Restore cursor using raw ANSI (can't use Term in signal handler)
                let _ = std::io::stderr().write_all(b"\x1b[?25h");
                let _ = std::io::stderr().flush();
            }
            std::process::exit(130); // 128 + SIGINT(2)
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
