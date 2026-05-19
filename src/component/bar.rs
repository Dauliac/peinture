//! Bar component — animated pulse indicator.
//!
//! ```rust,ignore
//! let bar = Bar::pulse()
//!     .frames(&["▐▌", "▐▋", "▐▊", "▐▉"])
//!     .home(0)
//!     .cycle_ms(3200)
//!     .color_a(Semantic::Info)
//!     .color_b(Semantic::Primary);
//! ```

use crate::component::Frame;
use crate::tokens::{Semantic, Theme};
use crate::traits::{Animate, Render};
use std::time::Instant;

/// An animated bar that pulses between frames.
#[derive(Debug, Clone)]
pub struct Bar {
    /// Ordered frames from smallest to largest.
    frames: Vec<String>,
    /// Index of the home/resting frame.
    home_idx: usize,
    /// Cycle duration in milliseconds.
    cycle_ms: u32,
    /// Start time for animation.
    start: Instant,
    /// Whether animation is active.
    active: bool,
    /// Color at home position.
    color_a: Semantic,
    /// Color at peak displacement.
    color_b: Semantic,
}

impl Bar {
    /// Create a pulse bar with default beacon settings from theme.
    pub fn from_theme(theme: &Theme) -> Self {
        Self {
            frames: theme.beacon.bar_frames.clone(),
            home_idx: theme.beacon.bar_home_idx,
            cycle_ms: theme.beacon.pulse_cycle_ms,
            start: Instant::now(),
            active: true,
            color_a: Semantic::PulseA,
            color_b: Semantic::PulseB,
        }
    }

    /// Create a static (non-animated) bar.
    pub fn static_bar(frame: impl Into<String>, color: Semantic) -> Self {
        let f: String = frame.into();
        Self {
            frames: vec![f],
            home_idx: 0,
            cycle_ms: 0,
            start: Instant::now(),
            active: false,
            color_a: color,
            color_b: color,
        }
    }

    /// Set the animation start time (for shared timing across frames).
    pub fn start_at(mut self, instant: Instant) -> Self {
        self.start = instant;
        self
    }

    /// Set whether animation is active.
    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// Override cycle duration.
    pub fn cycle_ms(mut self, ms: u32) -> Self {
        self.cycle_ms = ms;
        self
    }

    /// Current displacement (0.0 = home, 1.0 = max).
    fn displacement(&self) -> f32 {
        if !self.active || self.cycle_ms == 0 {
            return 0.0;
        }
        let elapsed_ms = self.start.elapsed().as_millis() as u32;
        let phase = (elapsed_ms % self.cycle_ms) as f32 / self.cycle_ms as f32;
        heartbeat(phase)
    }

    /// Map displacement to frame index.
    fn current_frame_idx(&self) -> usize {
        let d = self.displacement();
        let home = self.home_idx as f32;
        let max_idx = (self.frames.len() - 1) as f32;

        let idx = if d >= 0.0 {
            home + d * (max_idx - home)
        } else {
            (home + d * home).max(0.0)
        };

        idx.round().clamp(0.0, max_idx) as usize
    }
}

impl Render for Bar {
    fn render(&self, theme: &Theme) -> Frame {
        let idx = self.current_frame_idx();
        let bar_str = &self.frames[idx];

        let d = self.displacement();
        let color_a = self.color_a.resolve(&theme.palette);
        let color_b = self.color_b.resolve(&theme.palette);
        let color = color_a.lerp(&color_b, d.abs());

        Frame::line(format!("{}{}\x1b[0m", color.fg_code(), bar_str))
    }
}

impl Animate for Bar {
    fn is_active(&self) -> bool {
        self.active
    }

    fn is_at_rest(&self) -> bool {
        if !self.active || self.cycle_ms == 0 {
            return true;
        }
        let elapsed_ms = self.start.elapsed().as_millis() as u32;
        let phase = (elapsed_ms % self.cycle_ms) as f32 / self.cycle_ms as f32;
        // Rest phase: 0.50–1.00
        phase >= 0.50
    }
}

/// Heartbeat easing — expand right only.
///
/// Returns displacement from 0.0 (home) to +1.0 (peak).
/// The bar grows rightward from █ into the second column.
///
/// ```text
///  +1.0  ┃      ╭╮
///        ┃     ╱  ╲
///   0.0  ┃───╱──────╲───────────────────
///        ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
///        0%       25%   50%         100%
///          expand  return    rest
/// ```
fn heartbeat(phase: f32) -> f32 {
    match phase {
        // Expand: home → peak (grow right)
        p if p < 0.25 => {
            let t = p / 0.25;
            ease_out_quad(t)
        }
        // Return: peak → home
        p if p < 0.50 => {
            let t = (p - 0.25) / 0.25;
            1.0 - ease_in_quad(t)
        }
        // Rest at home
        _ => 0.0,
    }
}

fn ease_out_quad(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}

fn ease_in_quad(t: f32) -> f32 {
    t * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_bar_renders() {
        let bar = Bar::static_bar("▐▌", Semantic::Success);
        let theme = Theme::default();
        let frame = bar.render(&theme);
        assert!(frame.lines[0].contains("▐▌"));
    }

    #[test]
    fn from_theme_creates_animated() {
        let theme = Theme::default();
        let bar = Bar::from_theme(&theme);
        assert!(bar.is_active());
    }

    #[test]
    fn inactive_bar_at_rest() {
        let bar = Bar::static_bar("▐▌", Semantic::Success).active(false);
        assert!(bar.is_at_rest());
    }

    #[test]
    fn heartbeat_bounds() {
        assert!(heartbeat(0.0).abs() < 0.01);
        assert!(heartbeat(0.20) > 0.8);       // expand peak
        assert!(heartbeat(0.60).abs() < 0.01); // rest
        assert!(heartbeat(0.95).abs() < 0.01); // rest
    }

    #[test]
    fn heartbeat_never_negative() {
        for i in 0..100 {
            assert!(heartbeat(i as f32 / 100.0) >= -0.01);
        }
    }
}
