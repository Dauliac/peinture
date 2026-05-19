//! Pulse animation — breathing heartbeat bar.
//!
//! The pulse oscillates around the **medium** bar character (home position).
//! During one cycle: medium → expand (large) → medium → contract (small) → medium.
//! This creates a natural heartbeat/breathing feel.

use crate::tokens::{Color, Theme};
use std::f32::consts::PI;
use std::time::Instant;

/// A single frame of the pulse animation.
#[derive(Debug, Clone)]
pub struct PulseFrame {
    /// The bar character for this frame.
    pub bar_char: char,
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
    ///
    /// bar_chars layout: [0]=large (expand), [1]=medium (home), [2]=small (contract)
    ///
    /// Cycle: medium → large → medium → small → medium
    /// Using sine wave: sin goes 0 → +1 → 0 → -1 → 0
    /// Mapped to: medium → large → medium → small → medium
    pub fn frame(&self, theme: &Theme) -> PulseFrame {
        let tokens = &theme.beacon;
        let elapsed_ms = self.start.elapsed().as_millis() as u32;

        // No animation: return medium (home)
        if tokens.pulse_cycle_ms == 0 {
            return PulseFrame {
                bar_char: tokens.bar_chars[1],
                color: theme.palette.pulse_a,
            };
        }

        let phase = (elapsed_ms % tokens.pulse_cycle_ms) as f32 / tokens.pulse_cycle_ms as f32;
        // Sine wave: -1..+1 (centered on 0 = medium)
        let sine = (phase * 2.0 * PI).sin();

        // Map sine to bar character:
        //   sine > 0.3  → large (expand)
        //   sine < -0.3 → small (contract)
        //   else         → medium (home)
        let bar_idx = if sine > 0.3 {
            0 // large (expand)
        } else if sine < -0.3 {
            2 // small (contract)
        } else {
            1 // medium (home)
        };

        // Color: lerp between pulse_a and pulse_b based on absolute displacement
        // At home (sine~0): pulse_a. At extremes (sine~+-1): pulse_b.
        let color_t = sine.abs();

        PulseFrame {
            bar_char: tokens.bar_chars[bar_idx],
            color: theme.palette.pulse_a.lerp(&theme.palette.pulse_b, color_t),
        }
    }

    /// Render the bar character with its current color.
    pub fn render_bar(&self, theme: &Theme) -> String {
        let frame = self.frame(theme);
        format!("{}{}\x1b[0m", frame.color.fg_code(), frame.bar_char)
    }

    /// Render a static (non-animated) bar with a specific color.
    /// Uses the medium (home) bar character by default.
    pub fn render_static(bar_char: char, color: &Color) -> String {
        format!("{}{}\x1b[0m", color.fg_code(), bar_char)
    }

    /// Render the medium (home) bar with a specific color.
    pub fn render_home(theme: &Theme, color: &Color) -> String {
        Self::render_static(theme.beacon.bar_chars[1], color)
    }
}

impl Default for Pulse {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pulse_frame_returns_valid_bar() {
        let theme = Theme::default();
        let pulse = Pulse::new();
        let frame = pulse.frame(&theme);
        assert!(theme.beacon.bar_chars.contains(&frame.bar_char));
    }

    #[test]
    fn no_animation_returns_medium() {
        let mut theme = Theme::default();
        theme.beacon.pulse_cycle_ms = 0;
        let pulse = Pulse::new();
        let frame = pulse.frame(&theme);
        assert_eq!(frame.bar_char, theme.beacon.bar_chars[1]);
    }

    #[test]
    fn static_bar_renders() {
        let color = Color::Named(crate::tokens::palette::NamedColor::Green);
        let s = Pulse::render_static('\u{258A}', &color);
        assert!(s.contains('\u{258A}'));
    }
}
