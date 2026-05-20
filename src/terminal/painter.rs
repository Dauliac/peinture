//! Painter — two-phase renderer.
//!
//! **Phase 1 (Filling)**: Beacon starts right below cursor. Stream content
//! pushes it down. No scroll region — uses nom-style cursor-up/redraw.
//! No blank lines between prompt and beacon.
//!
//! **Phase 2 (Pinned)**: When screen fills, scroll region locks the beacon
//! at the bottom. Stream scrolls above. No blink.
//!
//! Transition happens automatically when total printed lines >= term_height.

use console::Term;
use std::collections::VecDeque;
use std::io::Write;
use std::sync::atomic::{AtomicBool, Ordering};
use super::sync_update::{SYNC_BEGIN, SYNC_END};

static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    /// Beacon follows content, no scroll region. Screen not yet full.
    Filling,
    /// Beacon pinned at bottom, scroll region active.
    Pinned,
}

pub struct Painter {
    term: Term,
    phase: Phase,
    /// Beacon lines from last frame (for cursor-up in Filling phase).
    pinned_line_count: usize,
    /// Total stream lines printed so far (for Filling→Pinned transition).
    stream_lines_total: usize,
    /// FIFO buffer for stream lines.
    buffer: VecDeque<String>,
    reserve: usize,
    cursor_hidden: bool,
    term_width: u16,
    term_height: u16,
    prev_term_height: u16,
}

impl Painter {
    pub fn new(term_width: u16, term_height: u16, reserve: usize) -> Self {
        Self {
            term: Term::stderr(),
            phase: Phase::Filling,
            pinned_line_count: 0,
            stream_lines_total: 0,
            buffer: VecDeque::new(),
            reserve,
            cursor_hidden: false,
            term_width,
            term_height,
            prev_term_height: term_height,
        }
    }

    pub fn stream_line(&mut self, line: String) {
        self.buffer.push_back(line);
    }

    pub fn stream_lines(&mut self, lines: impl IntoIterator<Item = String>) {
        self.buffer.extend(lines);
    }

    pub fn render_frame(&mut self, pinned_lines: &[String]) {
        // Detect resize
        let (new_h, new_w) = self.term.size();
        let resized = new_w != self.term_width || new_h != self.term_height;
        if resized {
            self.prev_term_height = self.term_height;
            self.term_width = new_w;
            self.term_height = new_h;
            // If pinned and resize: handle scroll region change
            if self.phase == Phase::Pinned {
                self.handle_resize(pinned_lines);
                return;
            }
            // If filling: just update dimensions, continue normally
        }

        // Truncate lines
        let tw = self.term_width;
        let pinned_lines: Vec<String> = pinned_lines.iter()
            .map(|l| truncate_line(l, tw))
            .collect();

        let new_height = pinned_lines.len() as u16;

        match self.phase {
            Phase::Filling => self.render_filling(&pinned_lines, new_height),
            Phase::Pinned => self.render_pinned(&pinned_lines, new_height),
        }
    }

    /// Phase 1: beacon follows content, no scroll region.
    fn render_filling(&mut self, pinned_lines: &[String], new_height: u16) {
        let beacon_growing = pinned_lines.len() > self.pinned_line_count;

        // When beacon grows: DON'T flush stream this frame.
        // Stream lines overwrite the first beacon line (notification),
        // causing a visible flash. Let the beacon grow cleanly first,
        // stream flushes on the next frame.
        let flushed: Vec<String> = if beacon_growing {
            Vec::new()
        } else {
            let overflow = self.buffer.len().saturating_sub(self.reserve);
            let to_flush = overflow.min(self.buffer.len());
            let tw = self.term_width;
            self.buffer.drain(..to_flush)
                .map(|l| truncate_line(&l, tw))
                .collect()
        };

        let mut buf = Vec::<u8>::with_capacity(4096);
        buf.extend_from_slice(SYNC_BEGIN.as_bytes());

        // Move cursor to start of previous beacon
        if self.pinned_line_count > 1 {
            buf.extend_from_slice(format!("\x1b[{}F", self.pinned_line_count - 1).as_bytes());
        } else if self.pinned_line_count == 1 {
            buf.extend_from_slice(b"\r");
        }

        // Write stream lines (push beacon down)
        for line in &flushed {
            buf.extend_from_slice(line.as_bytes());
            buf.extend_from_slice(b"\x1b[K\n");
            self.stream_lines_total += 1;
        }

        // Write beacon lines (overwrite old beacon position)
        for (i, line) in pinned_lines.iter().enumerate() {
            buf.extend_from_slice(line.as_bytes());
            buf.extend_from_slice(b"\x1b[K");
            if i < pinned_lines.len() - 1 {
                buf.extend_from_slice(b"\n");
            }
        }

        buf.extend_from_slice(SYNC_END.as_bytes());

        let _ = self.term.write_all(&buf);
        let _ = self.term.flush();

        self.pinned_line_count = pinned_lines.len();

        // Transition to Pinned ASAP — as soon as we have stream content
        // or beacon grows beyond 1 line. Scroll regions don't blink.
        // Nom-style overwrite blinks on terminals without sync update support.
        if self.stream_lines_total > 0 || self.pinned_line_count > 1 {
            self.transition_to_pinned(new_height);
        }
    }

    /// Transition from Filling to Pinned: set scroll region.
    fn transition_to_pinned(&mut self, beacon_height: u16) {
        let scroll_bottom = self.term_height.saturating_sub(beacon_height);
        let mut buf = String::new();
        buf.push_str(&format!("\x1b[1;{scroll_bottom}r"));
        buf.push_str(&format!("\x1b[{scroll_bottom};1H"));
        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();
        self.phase = Phase::Pinned;
    }

    /// Phase 2: beacon pinned at bottom, scroll region active.
    fn render_pinned(&mut self, pinned_lines: &[String], new_height: u16) {
        let old_height = self.pinned_line_count as u16;

        // Buffer flush with gap fill
        let gap = if new_height < old_height {
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

        if new_height != old_height {
            // Beacon height changed — resize scroll region
            buf.push_str("\x1b[r");

            let old_start = self.term_height.saturating_sub(old_height) + 1;
            let new_start = self.term_height.saturating_sub(new_height) + 1;
            let mut row = old_start;

            let gap_lines = &flushed[..gap.min(flushed.len())];
            for line in gap_lines {
                buf.push_str(&format!("\x1b[{row};1H{line}\x1b[K"));
                row += 1;
            }
            while row < new_start {
                buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
                row += 1;
            }

            for (i, line) in pinned_lines.iter().enumerate() {
                let r = new_start + i as u16;
                buf.push_str(&format!("\x1b[{r};1H{line}\x1b[K"));
            }

            let scroll_bottom = self.term_height.saturating_sub(new_height);
            buf.push_str(&format!("\x1b[1;{scroll_bottom}r"));
            buf.push_str(&format!("\x1b[{scroll_bottom};1H"));

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

        // Normal pinned frame
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

    /// Handle resize while in Pinned phase.
    fn handle_resize(&mut self, pinned_lines: &[String]) {
        let tw = self.term_width;
        let pinned_lines: Vec<String> = pinned_lines.iter()
            .map(|l| truncate_line(l, tw))
            .collect();
        let new_height = pinned_lines.len() as u16;
        let old_height = self.pinned_line_count as u16;

        let mut buf = String::with_capacity(2048);
        buf.push_str(SYNC_BEGIN);
        buf.push_str("\x1b[r"); // reset scroll region

        // On grow: clear old beacon ghost
        if self.term_height > self.prev_term_height {
            let old_beacon_start = self.prev_term_height.saturating_sub(old_height) + 1;
            for row in old_beacon_start..=self.prev_term_height {
                buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
            }
        }

        // Clear + draw new beacon area
        let new_beacon_start = self.term_height.saturating_sub(new_height) + 1;
        for row in new_beacon_start..=self.term_height {
            buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
        }
        for (i, line) in pinned_lines.iter().enumerate() {
            let row = new_beacon_start + i as u16;
            buf.push_str(&format!("\x1b[{row};1H{line}\x1b[K"));
        }

        // New scroll region
        let scroll_bottom = self.term_height.saturating_sub(new_height);
        buf.push_str(&format!("\x1b[1;{scroll_bottom}r"));
        buf.push_str(&format!("\x1b[{scroll_bottom};1H"));
        buf.push_str(SYNC_END);

        let _ = self.term.write_all(buf.as_bytes());
        let _ = self.term.flush();
        self.pinned_line_count = pinned_lines.len();
    }

    pub fn clear_pinned(&mut self) {
        match self.phase {
            Phase::Pinned => {
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
                }
                let _ = self.term.write_all(b"\x1b[r");
                let _ = self.term.flush();
            }
            Phase::Filling => {
                // Nom-style: cursor up + clear
                if self.pinned_line_count > 1 {
                    let _ = self.term.write_all(
                        format!("\x1b[{}F\x1b[J", self.pinned_line_count - 1).as_bytes()
                    );
                } else if self.pinned_line_count == 1 {
                    let _ = self.term.write_all(b"\r\x1b[J");
                }
                let _ = self.term.flush();
            }
        }
        self.pinned_line_count = 0;
    }

    pub fn print_final(&mut self, lines: &[String]) {
        // Flush remaining buffer
        if !self.buffer.is_empty() {
            if self.phase == Phase::Pinned {
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
            } else {
                // Filling: cursor up past beacon, print buffer, then beacon again
                // Just flush all to the beacon's print_final
                self.buffer.clear();
            }
        }
        self.clear_pinned();
        if self.phase == Phase::Pinned {
            let _ = self.term.write_all(format!("\x1b[{};1H", self.term_height).as_bytes());
            let _ = self.term.flush();
        }
        for line in lines {
            let _ = self.term.write_line(line);
        }
        self.show_cursor();
        self.phase = Phase::Filling;
        self.stream_lines_total = 0;
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

    #[test]
    fn starts_in_filling_phase() {
        let p = Painter::new(80, 24, 6);
        assert_eq!(p.phase, Phase::Filling);
    }
}
