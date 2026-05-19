//! Semantic color names — the bridge between components and the palette.
//!
//! Components reference colors by meaning, not by value.
//! The Theme resolves semantics to actual colors.

use serde::Deserialize;
use super::palette::{Color, Palette};

/// Semantic color reference — resolved against a Palette at render time.
///
/// This is peinture's equivalent of Tailwind utility classes.
/// Components use `Semantic::Success` instead of `Color::green()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Semantic {
    Success,
    Error,
    Warning,
    Info,
    Muted,
    Primary,
    Secondary,
    PulseA,
    PulseB,
    BarIdle,
    BarError,
    BarWarning,
}

impl Semantic {
    /// Resolve this semantic color against a concrete palette.
    pub fn resolve(&self, palette: &Palette) -> Color {
        match self {
            Self::Success => palette.success,
            Self::Error => palette.error,
            Self::Warning => palette.warning,
            Self::Info => palette.info,
            Self::Muted => palette.muted,
            Self::Primary => palette.primary,
            Self::Secondary => palette.secondary,
            Self::PulseA => palette.pulse_a,
            Self::PulseB => palette.pulse_b,
            Self::BarIdle => palette.bar_idle,
            Self::BarError => palette.bar_error,
            Self::BarWarning => palette.bar_warning,
        }
    }
}
