//! Pulse animation — breathing heartbeat bar.

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
    pub fn frame(&self, theme: &Theme) -> PulseFrame {
        let tokens = &theme.beacon;
        let elapsed_ms = self.start.elapsed().as_millis() as u32;

        if tokens.pulse_cycle_ms == 0 {
            return PulseFrame {
                bar_char: tokens.bar_chars[0],
                color: theme.palette.pulse_a,
            };
        }

        let phase = (elapsed_ms % tokens.pulse_cycle_ms) as f32 / tokens.pulse_cycle_ms as f32;
        // Sine wave: 0..1..0 (smooth breathing)
        let t = ((phase * 2.0 * PI).sin() + 1.0) / 2.0;

        // Bar thickness: thick at rest (t=0), thin at peak (t=1)
        let bar_idx = if t < 0.33 {
            0 // thick (resting)
        } else if t < 0.66 {
            1 // medium
        } else {
            2 // thin (peak)
        };

        PulseFrame {
            bar_char: tokens.bar_chars[bar_idx],
            color: theme.palette.pulse_a.lerp(&theme.palette.pulse_b, t),
        }
    }

    /// Render the bar character with its current color.
    pub fn render_bar(&self, theme: &Theme) -> String {
        let frame = self.frame(theme);
        format!("{}{}\x1b[0m", frame.color.fg_code(), frame.bar_char)
    }

    /// Render a static (non-animated) bar with a specific color.
    pub fn render_static(bar_char: char, color: &Color) -> String {
        format!("{}{}\x1b[0m", color.fg_code(), bar_char)
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
    fn static_bar_renders() {
        let color = Color::Named(crate::tokens::palette::NamedColor::Green);
        let s = Pulse::render_static('\u{258E}', &color);
        assert!(s.contains('\u{258E}'));
    }
}
