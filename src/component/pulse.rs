//! Pulse animation — heartbeat with centered expansion.
//!
//! The heartbeat curve has two bumps per cycle like a real cardiac rhythm:
//!
//! ```text
//!  +1.0  ┃      ╭╮
//!        ┃     ╱  ╲
//!        ┃    ╱    ╲
//!   0.0  ┃───╱──────╲──────────╱────────
//!        ┃                    ╱
//!  -0.7  ┃              ╰──╯
//!        ┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
//!        0%       25%    50%   75%   100%
//!          expand   return contract rest
//! ```
//!
//! At 12fps with a 2400ms cycle = 28 frames per beat.
//! The expand phase uses ~8 frames to go from home(3) to max(6) = ~2 frames per stage.
//! Every intermediate bar frame is displayed.

use crate::tokens::{Color, Theme};
use std::time::Instant;

/// A single frame of the pulse animation.
#[derive(Debug, Clone)]
pub struct PulseFrame {
    /// The bar string for this frame (multi-char, centered).
    pub bar: String,
    /// The interpolated color for this frame.
    pub color: Color,
}

/// Pulse animation state.
pub struct Pulse {
    start: Instant,
}

impl Pulse {
    /// Create a new pulse starting now.
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }

    /// Compute the current animation frame.
    pub fn frame(&self, theme: &Theme) -> PulseFrame {
        let tokens = &theme.beacon;
        let elapsed_ms = self.start.elapsed().as_millis() as u32;

        if tokens.pulse_cycle_ms == 0 {
            return PulseFrame {
                bar: tokens.home_frame().to_string(),
                color: theme.palette.pulse_a,
            };
        }

        let phase = (elapsed_ms % tokens.pulse_cycle_ms) as f32 / tokens.pulse_cycle_ms as f32;
        let displacement = heartbeat(phase);

        let bar = tokens.frame_for_displacement(displacement).to_string();
        let color_t = displacement.abs();

        PulseFrame {
            bar,
            color: theme.palette.pulse_a.lerp(&theme.palette.pulse_b, color_t),
        }
    }

    /// Render the bar string with its current color.
    pub fn render_bar(&self, theme: &Theme) -> String {
        let frame = self.frame(theme);
        format!("{}{}\x1b[0m", frame.color.fg_code(), frame.bar)
    }

    /// Whether the pulse is currently at the home/rest position.
    /// Use this to know when it's safe to stop the animation cleanly.
    pub fn is_at_rest(&self, theme: &Theme) -> bool {
        let tokens = &theme.beacon;
        if tokens.pulse_cycle_ms == 0 {
            return true;
        }
        let elapsed_ms = self.start.elapsed().as_millis() as u32;
        let phase = (elapsed_ms % tokens.pulse_cycle_ms) as f32 / tokens.pulse_cycle_ms as f32;
        // Rest phase: 0.50–1.00
        phase >= 0.50
    }

    /// Milliseconds until the next rest phase.
    /// Returns 0 if already at rest.
    pub fn ms_until_rest(&self, theme: &Theme) -> u64 {
        let tokens = &theme.beacon;
        if tokens.pulse_cycle_ms == 0 {
            return 0;
        }
        let elapsed_ms = self.start.elapsed().as_millis() as u32;
        let phase = (elapsed_ms % tokens.pulse_cycle_ms) as f32 / tokens.pulse_cycle_ms as f32;

        if phase >= 0.50 {
            return 0;
        }

        let target = 0.50;
        let remaining_phase = target - phase;
        (remaining_phase * tokens.pulse_cycle_ms as f32) as u64
    }

    /// Render a static bar with a specific color.
    pub fn render_static(bar: &str, color: &Color) -> String {
        format!("{}{}\x1b[0m", color.fg_code(), bar)
    }

    /// Render the medium (home) bar with a specific color.
    pub fn render_home(theme: &Theme, color: &Color) -> String {
        Self::render_static(theme.beacon.home_frame(), color)
    }
}

impl Default for Pulse {
    fn default() -> Self {
        Self::new()
    }
}

/// Heartbeat easing curve — expand-only.
///
/// Returns displacement from 0.0 (home/minimum) to +1.0 (largest).
/// No contraction below home since ▐▌ is the minimum bar.
///
/// Timing budget (3200ms cycle, 12fps = ~38 frames):
///   0.00–0.25  expand    home→max       ~10 frames  (ease-out-quad)
///   0.25–0.50  return    max→home       ~10 frames  (ease-in-quad)
///   0.50–1.00  rest      home           ~18 frames
///
/// The long rest gives the heartbeat a calm, organic rhythm.
fn heartbeat(phase: f32) -> f32 {
    match phase {
        // Expand: home → max (snappy attack)
        p if p < 0.25 => {
            let t = p / 0.25;
            ease_out_quad(t)
        }
        // Return: max → home (gradual release)
        p if p < 0.50 => {
            let t = (p - 0.25) / 0.25;
            1.0 - ease_in_quad(t)
        }
        // Rest at home
        _ => 0.0,
    }
}

/// Ease-out quadratic: snappy start, gentle end.
fn ease_out_quad(t: f32) -> f32 {
    1.0 - (1.0 - t) * (1.0 - t)
}

/// Ease-in quadratic: gentle start, snappy end.
fn ease_in_quad(t: f32) -> f32 {
    t * t
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_frame_returns_valid_bar() {
        let theme = Theme::default();
        let pulse = Pulse::new();
        let frame = pulse.frame(&theme);
        assert!(theme.beacon.bar_frames.contains(&frame.bar));
    }

    #[test]
    fn no_animation_returns_home() {
        let mut theme = Theme::default();
        theme.beacon.pulse_cycle_ms = 0;
        let pulse = Pulse::new();
        let frame = pulse.frame(&theme);
        assert_eq!(frame.bar, theme.beacon.home_frame());
    }

    #[test]
    fn heartbeat_starts_at_zero() {
        assert!(heartbeat(0.0).abs() < 0.01);
    }

    #[test]
    fn heartbeat_reaches_peak() {
        assert!(heartbeat(0.20) > 0.8);
    }

    #[test]
    fn heartbeat_never_negative() {
        // Expand-only: displacement is always >= 0
        for i in 0..100 {
            let phase = i as f32 / 100.0;
            assert!(heartbeat(phase) >= -0.01, "phase {phase} was {}", heartbeat(phase));
        }
    }

    #[test]
    fn heartbeat_rests_at_end() {
        assert!(heartbeat(0.60).abs() < 0.01);
        assert!(heartbeat(0.95).abs() < 0.01);
    }

    #[test]
    fn all_frames_visited_during_expand() {
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        let theme = Theme::default();
        // 38 frames per cycle at 12fps / 3200ms
        for i in 0..39 {
            let phase = i as f32 / 38.0;
            let d = heartbeat(phase);
            let frame = theme.beacon.frame_for_displacement(d);
            visited.insert(frame.to_string());
        }
        // Should visit all 4 frames
        assert_eq!(visited.len(), 4, "Only visited {} frames: {:?}", visited.len(), visited);
    }

    #[test]
    fn ease_boundaries() {
        assert!(ease_out_quad(0.0).abs() < 0.001);
        assert!((ease_out_quad(1.0) - 1.0).abs() < 0.001);
        assert!(ease_in_quad(0.0).abs() < 0.001);
        assert!((ease_in_quad(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn static_bar_renders() {
        let color = Color::Named(crate::tokens::palette::NamedColor::Green);
        let s = Pulse::render_static("▐▌", &color);
        assert!(s.contains("▐▌"));
    }
}
