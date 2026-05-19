//! Color palette — primitive and semantic color tokens.

use serde::Deserialize;

/// A terminal color that can be rendered as ANSI escape codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum Color {
    /// ANSI 256-color palette index (0-255).
    Ansi256(u8),
    /// 24-bit truecolor RGB.
    Rgb(u8, u8, u8),
    /// Basic ANSI color by name.
    Named(NamedColor),
    /// No color (for plain/NO_COLOR mode).
    None,
}

/// Basic ANSI named colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum NamedColor {
    Black,
    Red,
    Green,
    Yellow,
    Blue,
    Magenta,
    Cyan,
    White,
    BrightBlack,
    BrightRed,
    BrightGreen,
    BrightYellow,
    BrightBlue,
    BrightMagenta,
    BrightCyan,
    BrightWhite,
}

impl Color {
    /// Render as ANSI foreground escape code (without reset).
    pub fn fg_code(&self) -> String {
        match self {
            Self::Ansi256(n) => format!("\x1b[38;5;{n}m"),
            Self::Rgb(r, g, b) => format!("\x1b[38;2;{r};{g};{b}m"),
            Self::Named(c) => format!("\x1b[{}m", c.fg_code()),
            Self::None => String::new(),
        }
    }

    /// Render as ANSI background escape code (without reset).
    pub fn bg_code(&self) -> String {
        match self {
            Self::Ansi256(n) => format!("\x1b[48;5;{n}m"),
            Self::Rgb(r, g, b) => format!("\x1b[48;2;{r};{g};{b}m"),
            Self::Named(c) => format!("\x1b[{}m", c.bg_code()),
            Self::None => String::new(),
        }
    }

    /// Linear interpolation between two colors for animation.
    /// `t` is 0.0..1.0 where 0.0 = self, 1.0 = other.
    pub fn lerp(&self, other: &Self, t: f32) -> Self {
        let t = t.clamp(0.0, 1.0);
        match (self.to_rgb(), other.to_rgb()) {
            (Some((r1, g1, b1)), Some((r2, g2, b2))) => {
                let r = (r1 as f32 + (r2 as f32 - r1 as f32) * t) as u8;
                let g = (g1 as f32 + (g2 as f32 - g1 as f32) * t) as u8;
                let b = (b1 as f32 + (b2 as f32 - b1 as f32) * t) as u8;
                Self::Rgb(r, g, b)
            }
            _ => if t < 0.5 { *self } else { *other },
        }
    }

    fn to_rgb(&self) -> Option<(u8, u8, u8)> {
        match self {
            Self::Rgb(r, g, b) => Some((*r, *g, *b)),
            Self::Named(c) => Some(c.approx_rgb()),
            Self::Ansi256(n) => Some(ansi256_to_rgb(*n)),
            Self::None => None,
        }
    }
}

impl NamedColor {
    fn fg_code(&self) -> u8 {
        match self {
            Self::Black => 30,
            Self::Red => 31,
            Self::Green => 32,
            Self::Yellow => 33,
            Self::Blue => 34,
            Self::Magenta => 35,
            Self::Cyan => 36,
            Self::White => 37,
            Self::BrightBlack => 90,
            Self::BrightRed => 91,
            Self::BrightGreen => 92,
            Self::BrightYellow => 93,
            Self::BrightBlue => 94,
            Self::BrightMagenta => 95,
            Self::BrightCyan => 96,
            Self::BrightWhite => 97,
        }
    }

    fn bg_code(&self) -> u8 {
        self.fg_code() + 10
    }

    fn approx_rgb(&self) -> (u8, u8, u8) {
        match self {
            Self::Black => (0, 0, 0),
            Self::Red => (205, 49, 49),
            Self::Green => (13, 188, 121),
            Self::Yellow => (229, 229, 16),
            Self::Blue => (36, 114, 200),
            Self::Magenta => (188, 63, 188),
            Self::Cyan => (17, 168, 205),
            Self::White => (229, 229, 229),
            Self::BrightBlack => (102, 102, 102),
            Self::BrightRed => (241, 76, 76),
            Self::BrightGreen => (35, 209, 139),
            Self::BrightYellow => (245, 245, 67),
            Self::BrightBlue => (59, 142, 234),
            Self::BrightMagenta => (214, 112, 214),
            Self::BrightCyan => (41, 184, 219),
            Self::BrightWhite => (229, 229, 229),
        }
    }
}

/// Semantic color palette — maps meaning to colors.
#[derive(Debug, Clone, Deserialize)]
pub struct Palette {
    pub success: Color,
    pub error: Color,
    pub warning: Color,
    pub info: Color,
    pub muted: Color,
    pub primary: Color,
    pub secondary: Color,

    /// Pastel rainbow for brand text (6 ANSI-256 codes).
    pub rainbow: Vec<Color>,

    /// Pulse animation start color (breathing).
    pub pulse_a: Color,
    /// Pulse animation end color (breathing peak).
    pub pulse_b: Color,

    /// Bar color when idle/success.
    pub bar_idle: Color,
    /// Bar color when error.
    pub bar_error: Color,
    /// Bar color when warning.
    pub bar_warning: Color,
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            success: Color::Named(NamedColor::Green),
            error: Color::Named(NamedColor::Red),
            warning: Color::Named(NamedColor::Yellow),
            info: Color::Named(NamedColor::Cyan),
            muted: Color::Rgb(90, 90, 90),        // RGB gray so fade lerp works smoothly
            primary: Color::Named(NamedColor::Cyan),
            secondary: Color::Named(NamedColor::White),

            // Pastel rainbow: pink, peach, cream, mint, sky, lavender
            rainbow: vec![
                Color::Ansi256(211),
                Color::Ansi256(216),
                Color::Ansi256(222),
                Color::Ansi256(158),
                Color::Ansi256(117),
                Color::Ansi256(147),
            ],

            // Truecolor gradient: deep blue (rest) → bright cyan (peak)
            pulse_a: Color::Rgb(60, 100, 180),
            pulse_b: Color::Rgb(80, 220, 255),

            bar_idle: Color::Rgb(130, 210, 160),     // pastel mint green (softer than ANSI)
            bar_error: Color::Named(NamedColor::Red),
            bar_warning: Color::Named(NamedColor::Yellow),
        }
    }
}

impl Palette {
    /// Plain palette — no colors at all.
    pub fn plain() -> Self {
        Self {
            success: Color::None,
            error: Color::None,
            warning: Color::None,
            info: Color::None,
            muted: Color::None,
            primary: Color::None,
            secondary: Color::None,
            rainbow: vec![Color::None; 6],
            pulse_a: Color::None,
            pulse_b: Color::None,
            bar_idle: Color::None,
            bar_error: Color::None,
            bar_warning: Color::None,
        }
    }
}

/// Convert ANSI 256-color index to approximate RGB.
fn ansi256_to_rgb(n: u8) -> (u8, u8, u8) {
    match n {
        0..=7 => {
            let colors: [(u8, u8, u8); 8] = [
                (0, 0, 0), (205, 49, 49), (13, 188, 121), (229, 229, 16),
                (36, 114, 200), (188, 63, 188), (17, 168, 205), (229, 229, 229),
            ];
            colors[n as usize]
        }
        8..=15 => {
            let colors: [(u8, u8, u8); 8] = [
                (102, 102, 102), (241, 76, 76), (35, 209, 139), (245, 245, 67),
                (59, 142, 234), (214, 112, 214), (41, 184, 219), (255, 255, 255),
            ];
            colors[(n - 8) as usize]
        }
        16..=231 => {
            let idx = n - 16;
            let b = (idx % 6) * 51;
            let g = ((idx / 6) % 6) * 51;
            let r = (idx / 36) * 51;
            (r, g, b)
        }
        232..=255 => {
            let v = 8 + (n - 232) * 10;
            (v, v, v)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fg_code_ansi256() {
        assert_eq!(Color::Ansi256(211).fg_code(), "\x1b[38;5;211m");
    }

    #[test]
    fn fg_code_rgb() {
        assert_eq!(Color::Rgb(255, 0, 128).fg_code(), "\x1b[38;2;255;0;128m");
    }

    #[test]
    fn fg_code_none() {
        assert_eq!(Color::None.fg_code(), "");
    }

    #[test]
    fn lerp_rgb() {
        let a = Color::Rgb(0, 0, 0);
        let b = Color::Rgb(100, 200, 100);
        let mid = a.lerp(&b, 0.5);
        assert_eq!(mid, Color::Rgb(50, 100, 50));
    }

    #[test]
    fn default_palette_has_rainbow() {
        let p = Palette::default();
        assert_eq!(p.rainbow.len(), 6);
    }
}
