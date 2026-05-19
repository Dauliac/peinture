//! Text component — styled text with semantic colors and decorations.
//!
//! ```rust,ignore
//! let t = Text::new("hello").color(Semantic::Success).bold();
//! let t = Text::dim("metadata");
//! let t = Text::rainbow("cimera");
//! ```

use crate::component::Frame;
use crate::tokens::{Semantic, Theme};
use crate::traits::Render;

/// A styled text span.
#[derive(Debug, Clone)]
pub struct Text {
    content: String,
    style: TextStyle,
}

/// How to style the text.
#[derive(Debug, Clone, Default)]
pub struct TextStyle {
    pub color: Option<Semantic>,
    pub bold: bool,
    pub dim: bool,
    pub rainbow: bool,
}

impl Text {
    /// Plain text (inherits color from context).
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            style: TextStyle::default(),
        }
    }

    /// Dim/muted text.
    pub fn dim(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            style: TextStyle { dim: true, ..Default::default() },
        }
    }

    /// Rainbow-colored text (per-character from palette).
    pub fn rainbow(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            style: TextStyle { rainbow: true, bold: true, ..Default::default() },
        }
    }

    /// Set semantic color.
    pub fn color(mut self, color: Semantic) -> Self {
        self.style.color = Some(color);
        self
    }

    /// Set bold.
    pub fn bold(mut self) -> Self {
        self.style.bold = true;
        self
    }

    /// Get the raw content.
    pub fn content(&self) -> &str {
        &self.content
    }
}

impl Render for Text {
    fn render(&self, theme: &Theme) -> Frame {
        let rendered = if self.style.rainbow {
            render_rainbow(&self.content, theme)
        } else {
            render_styled(&self.content, &self.style, theme)
        };
        Frame::line(rendered)
    }
}

fn render_rainbow(text: &str, theme: &Theme) -> String {
    let palette = &theme.palette;
    if palette.rainbow.is_empty() {
        return format!("\x1b[1m{text}\x1b[0m");
    }

    let mut out = String::new();
    for (i, ch) in text.chars().enumerate() {
        let color = &palette.rainbow[i % palette.rainbow.len()];
        let code = color.fg_code();
        // Strip outer \x1b[ and m to inline into the bold sequence
        let inner = code.trim_start_matches("\x1b[").trim_end_matches('m');
        out.push_str(&format!("\x1b[1;{inner}m{ch}"));
    }
    out.push_str("\x1b[0m");
    out
}

fn render_styled(text: &str, style: &TextStyle, theme: &Theme) -> String {
    let mut codes = Vec::new();

    if style.bold {
        codes.push("1".to_string());
    }
    if style.dim {
        codes.push("2".to_string());
    }
    if let Some(ref semantic) = style.color {
        let color = semantic.resolve(&theme.palette);
        let code = color.fg_code();
        let inner = code.trim_start_matches("\x1b[").trim_end_matches('m');
        if !inner.is_empty() {
            codes.push(inner.to_string());
        }
    }

    if codes.is_empty() {
        text.to_string()
    } else {
        format!("\x1b[{}m{}\x1b[0m", codes.join(";"), text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_no_ansi() {
        let t = Text::new("hello");
        let theme = Theme::plain();
        let frame = t.render(&theme);
        assert_eq!(frame.lines[0], "hello");
    }

    #[test]
    fn bold_text() {
        let t = Text::new("hello").bold();
        let theme = Theme::default();
        let frame = t.render(&theme);
        assert!(frame.lines[0].contains("\x1b[1m"));
    }

    #[test]
    fn rainbow_text() {
        let t = Text::rainbow("abc");
        let theme = Theme::default();
        let frame = t.render(&theme);
        assert!(frame.lines[0].contains('a'));
        assert!(frame.lines[0].contains('b'));
        assert!(frame.lines[0].contains('c'));
    }

    #[test]
    fn dim_text() {
        let t = Text::dim("meta");
        let theme = Theme::default();
        let frame = t.render(&theme);
        assert!(frame.lines[0].contains("\x1b[2m"));
    }
}
