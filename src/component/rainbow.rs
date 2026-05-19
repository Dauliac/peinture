//! Rainbow text renderer — per-character coloring from palette.

use crate::tokens::{Palette, Theme};

/// Render text with per-character rainbow coloring from the palette.
pub struct Rainbow;

impl Rainbow {
    /// Render `text` with rainbow colors, bold.
    /// Uses `palette.rainbow` colors cycling per character.
    pub fn render(text: &str, theme: &Theme) -> String {
        Self::render_with_palette(text, &theme.palette)
    }

    /// Render with a specific palette (for testing).
    pub fn render_with_palette(text: &str, palette: &Palette) -> String {
        if palette.rainbow.is_empty() || palette.rainbow.iter().all(|c| matches!(c, crate::tokens::Color::None)) {
            // No color mode — just bold
            return format!("\x1b[1m{text}\x1b[0m");
        }

        let mut out = String::new();
        for (i, ch) in text.chars().enumerate() {
            let color = &palette.rainbow[i % palette.rainbow.len()];
            out.push_str(&format!("\x1b[1;{}m{ch}", color.fg_code().trim_start_matches("\x1b[").trim_end_matches('m')));
        }
        out.push_str("\x1b[0m");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tokens::Color;

    #[test]
    fn rainbow_renders_each_char() {
        let palette = Palette {
            rainbow: vec![Color::Ansi256(211), Color::Ansi256(216)],
            ..Palette::default()
        };
        let result = Rainbow::render_with_palette("ab", &palette);
        assert!(result.contains("211"));
        assert!(result.contains("216"));
        assert!(result.contains('a'));
        assert!(result.contains('b'));
    }

    #[test]
    fn rainbow_plain_mode() {
        let palette = Palette::plain();
        let result = Rainbow::render_with_palette("test", &palette);
        assert_eq!(result, "\x1b[1mtest\x1b[0m");
    }
}
