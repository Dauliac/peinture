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

/// Heartbeat easing curve.
///
/// Returns continuous displacement from -1.0 to +1.0 (0.0 = home).
///
/// Timing budget (2400ms cycle, 12fps = ~28 frames):
///   0.00–0.28  expand    home→max       ~8 frames  (ease-out-quad)
///   0.28–0.52  return    max→home       ~7 frames  (ease-in-quad)
///   0.52–0.56  rest      home           ~1 frame
///   0.56–0.72  contract  home→min       ~5 frames  (ease-out-quad, weaker)
///   0.72–0.88  release   min→home       ~5 frames  (ease-in-quad)
///   0.88–1.00  rest      home           ~3 frames
fn heartbeat(phase: f32) -> f32 {
    match phase {
        // Expand: home → max (ease-out-quad for snappy start)
        p if p < 0.28 => {
            let t = p / 0.28;
            ease_out_quad(t)
        }
        // Return: max → home (ease-in-quad for gradual deceleration)
        p if p < 0.52 => {
            let t = (p - 0.28) / 0.24;
            1.0 - ease_in_quad(t)
        }
        // Brief rest at home
        p if p < 0.56 => 0.0,
        // Contract: home → min (softer, only 70% intensity)
        p if p < 0.72 => {
            let t = (p - 0.56) / 0.16;
            -0.7 * ease_out_quad(t)
        }
        // Release: min → home
        p if p < 0.88 => {
            let t = (p - 0.72) / 0.16;
            -0.7 * (1.0 - ease_in_quad(t))
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
        // Should be near 1.0 around phase 0.20-0.28
        assert!(heartbeat(0.25) > 0.8);
    }

    #[test]
    fn heartbeat_returns_to_home() {
        // Should be near 0 at phase 0.52-0.56
        assert!(heartbeat(0.54).abs() < 0.05);
    }

    #[test]
    fn heartbeat_contracts() {
        // Should go negative around 0.65-0.72
        assert!(heartbeat(0.68) < -0.4);
    }

    #[test]
    fn heartbeat_rests_at_end() {
        assert!(heartbeat(0.95).abs() < 0.01);
    }

    #[test]
    fn all_frames_visited_during_expand() {
        // Simulate 28 frames at 12fps over one 2400ms cycle
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        let theme = Theme::default();
        for i in 0..29 {
            let phase = i as f32 / 28.0;
            let d = heartbeat(phase);
            let frame = theme.beacon.frame_for_displacement(d);
            visited.insert(frame.to_string());
        }
        // Should visit at least 5 of the 7 frames (home + some expand + some contract)
        assert!(visited.len() >= 5, "Only visited {} frames: {:?}", visited.len(), visited);
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
