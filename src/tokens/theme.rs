//! Theme — the single source of truth for all visual properties.

use serde::Deserialize;
use super::{BeaconTokens, IconSet, Palette, Spacing, TreeChars};

/// Complete theme combining all token tiers.
///
/// Construct via `Theme::default()`, `Theme::plain()`, `Theme::ci()`,
/// or load from a TOML file with `Theme::from_toml()`.
#[derive(Debug, Clone, Deserialize)]
pub struct Theme {
    pub palette: Palette,
    pub spacing: Spacing,
    pub icons: IconSet,
    pub tree: TreeChars,
    pub beacon: BeaconTokens,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            palette: Palette::default(),
            spacing: Spacing::default(),
            icons: IconSet::default(),
            tree: TreeChars::default(),
            beacon: BeaconTokens::default(),
        }
    }
}

impl Theme {
    /// Plain theme — no colors, no unicode symbols.
    /// For NO_COLOR, TERM=dumb, or piped output.
    pub fn plain() -> Self {
        Self {
            palette: Palette::plain(),
            spacing: Spacing::default(),
            icons: IconSet::ascii(),
            tree: TreeChars::ascii(),
            beacon: BeaconTokens {
                fps: 0, // no animation
                ..BeaconTokens::default()
            },
        }
    }

    /// CI theme — colors preserved, no animations.
    pub fn ci() -> Self {
        Self {
            beacon: BeaconTokens {
                fps: 0, // no animation in CI
                ..BeaconTokens::default()
            },
            ..Self::default()
        }
    }

    /// Load theme overrides from a TOML string.
    /// Fields not present in the TOML keep their default values.
    pub fn from_toml(toml_str: &str) -> Result<Self, toml::de::Error> {
        toml::from_str(toml_str)
    }

    /// Whether animations are enabled (fps > 0).
    pub fn animations_enabled(&self) -> bool {
        self.beacon.fps > 0
    }

    /// ANSI reset code.
    pub fn reset() -> &'static str {
        "\x1b[0m"
    }

    /// Bold ANSI code.
    pub fn bold() -> &'static str {
        "\x1b[1m"
    }

    /// Dim ANSI code.
    pub fn dim() -> &'static str {
        "\x1b[2m"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_theme_has_animation() {
        let t = Theme::default();
        assert!(t.animations_enabled());
    }

    #[test]
    fn plain_theme_no_animation() {
        let t = Theme::plain();
        assert!(!t.animations_enabled());
    }

    #[test]
    fn ci_theme_no_animation() {
        let t = Theme::ci();
        assert!(!t.animations_enabled());
    }
}
