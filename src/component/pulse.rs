//! Pulse animation — heartbeat with centered expansion.
//!
//! The pulse uses a **heartbeat** easing curve:
//! - Quick expansion (systole) — ease-out
//! - Brief hold at peak
//! - Slow contraction back to home — ease-in
//! - Rest pause before next beat
//!
//! The bar expands from center outward using symmetric character pairs:
//! `▕▏` (thin) → `▐▌` (medium/home) → `██` (full)

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
    ///
    /// bar_frames: [0]=large (expand), [1]=medium (home), [2]=small (contract)
    ///
    /// Heartbeat cycle:
    /// ```text
    ///   0.00 - 0.15  quick expand    (home → large)    ease-out
    ///   0.15 - 0.25  hold at large
    ///   0.25 - 0.50  slow return     (large → home)    ease-in
    ///   0.50 - 0.60  slight contract (home → small)    ease-out
    ///   0.60 - 0.70  hold at small
    ///   0.70 - 0.85  return to home  (small → home)    ease-in
    ///   0.85 - 1.00  rest at home
    /// ```
    pub fn frame(&self, theme: &Theme) -> PulseFrame {
        let tokens = &theme.beacon;
        let elapsed_ms = self.start.elapsed().as_millis() as u32;

        // No animation: return medium (home)
        if tokens.pulse_cycle_ms == 0 {
            return PulseFrame {
                bar: tokens.bar_frames[1].clone(),
                color: theme.palette.pulse_a,
            };
        }

        let phase = (elapsed_ms % tokens.pulse_cycle_ms) as f32 / tokens.pulse_cycle_ms as f32;

        // Heartbeat curve: returns -1.0 (small) to +1.0 (large), 0.0 = home
        let displacement = heartbeat(phase);

        // Map displacement to bar frame
        let bar = if displacement > 0.3 {
            &tokens.bar_frames[0] // large (expand)
        } else if displacement < -0.3 {
            &tokens.bar_frames[2] // small (contract)
        } else {
            &tokens.bar_frames[1] // medium (home)
        };

        // Color intensity follows absolute displacement
        let color_t = displacement.abs();

        PulseFrame {
            bar: bar.clone(),
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
        Self::render_static(&theme.beacon.bar_frames[1], color)
    }
}

impl Default for Pulse {
    fn default() -> Self {
        Self::new()
    }
}

/// Heartbeat easing curve.
///
/// Returns displacement from -1.0 (contract) to +1.0 (expand), 0.0 = home.
///
/// The curve has two bumps per cycle (like a real heartbeat):
/// 1. A strong expansion bump (systole)
/// 2. A softer contraction bump (diastole)
/// 3. A rest period
fn heartbeat(phase: f32) -> f32 {
    // phase is 0.0..1.0 within one cycle
    match phase {
        // Quick expand: 0.00 → 0.12 (ease-out cubic)
        p if p < 0.12 => {
            let t = p / 0.12;
            ease_out_cubic(t)
        }
        // Hold at peak: 0.12 → 0.20
        p if p < 0.20 => 1.0,
        // Slow return: 0.20 → 0.45 (ease-in cubic)
        p if p < 0.45 => {
            let t = (p - 0.20) / 0.25;
            1.0 - ease_in_cubic(t)
        }
        // Brief rest at home: 0.45 → 0.50
        p if p < 0.50 => 0.0,
        // Soft contract: 0.50 → 0.60 (ease-out cubic, inverted, weaker)
        p if p < 0.60 => {
            let t = (p - 0.50) / 0.10;
            -0.7 * ease_out_cubic(t)
        }
        // Hold at contracted: 0.60 → 0.65
        p if p < 0.65 => -0.7,
        // Return to home: 0.65 → 0.80 (ease-in cubic)
        p if p < 0.80 => {
            let t = (p - 0.65) / 0.15;
            -0.7 * (1.0 - ease_in_cubic(t))
        }
        // Rest at home: 0.80 → 1.00
        _ => 0.0,
    }
}

/// Ease-out cubic: fast start, slow end. t in 0..1, returns 0..1.
fn ease_out_cubic(t: f32) -> f32 {
    let t1 = 1.0 - t;
    1.0 - t1 * t1 * t1
}

/// Ease-in cubic: slow start, fast end. t in 0..1, returns 0..1.
fn ease_in_cubic(t: f32) -> f32 {
    t * t * t
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
    fn no_animation_returns_medium() {
        let mut theme = Theme::default();
        theme.beacon.pulse_cycle_ms = 0;
        let pulse = Pulse::new();
        let frame = pulse.frame(&theme);
        assert_eq!(frame.bar, theme.beacon.bar_frames[1]);
    }

    #[test]
    fn heartbeat_starts_at_zero() {
        assert!((heartbeat(0.0)).abs() < 0.01);
    }

    #[test]
    fn heartbeat_peaks_positive() {
        // Should reach ~1.0 during expand phase
        assert!(heartbeat(0.15) > 0.9);
    }

    #[test]
    fn heartbeat_dips_negative() {
        // Should go negative during contract phase
        assert!(heartbeat(0.62) < -0.5);
    }

    #[test]
    fn heartbeat_rests_at_end() {
        assert!((heartbeat(0.95)).abs() < 0.01);
    }

    #[test]
    fn ease_out_cubic_boundaries() {
        assert!((ease_out_cubic(0.0)).abs() < 0.001);
        assert!((ease_out_cubic(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn ease_in_cubic_boundaries() {
        assert!((ease_in_cubic(0.0)).abs() < 0.001);
        assert!((ease_in_cubic(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn static_bar_renders() {
        let color = Color::Named(crate::tokens::palette::NamedColor::Green);
        let s = Pulse::render_static("▐▌", &color);
        assert!(s.contains("▐▌"));
    }
}
