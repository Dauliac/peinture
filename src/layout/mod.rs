//! Layout containers — composable positioning of components.
//!
//! Containers implement `Render` and compose child components.
//!
//! ```rust,ignore
//! let layout = VStack::new()
//!     .child(Text::new("header").bold())
//!     .child(Text::dim("body"))
//!     .child(Text::new("footer"));
//! ```

use crate::component::Frame;
use crate::tokens::Theme;
use crate::traits::Render;

/// Vertical stack — children stacked top-to-bottom.
pub struct VStack {
    children: Vec<Box<dyn Render>>,
}

impl VStack {
    pub fn new() -> Self {
        Self { children: Vec::new() }
    }

    /// Add a child component.
    pub fn child(mut self, child: impl Render + 'static) -> Self {
        self.children.push(Box::new(child));
        self
    }
}

impl Default for VStack {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for VStack {
    fn render(&self, theme: &Theme) -> Frame {
        let mut frame = Frame::new();
        for child in &self.children {
            frame.extend(&child.render(theme));
        }
        frame
    }
}

/// Horizontal stack — children placed side-by-side with a gap.
pub struct HStack {
    children: Vec<Box<dyn Render>>,
    gap: usize,
}

impl HStack {
    pub fn new() -> Self {
        Self { children: Vec::new(), gap: 0 }
    }

    /// Set the gap between children (in characters).
    pub fn gap(mut self, gap: usize) -> Self {
        self.gap = gap;
        self
    }

    /// Add a child component.
    pub fn child(mut self, child: impl Render + 'static) -> Self {
        self.children.push(Box::new(child));
        self
    }
}

impl Default for HStack {
    fn default() -> Self {
        Self::new()
    }
}

impl Render for HStack {
    fn render(&self, theme: &Theme) -> Frame {
        if self.children.is_empty() {
            return Frame::new();
        }

        let frames: Vec<Frame> = self.children.iter().map(|c| c.render(theme)).collect();
        let max_height = frames.iter().map(|f| f.height()).max().unwrap_or(0);
        let gap_str = " ".repeat(self.gap);

        let widths: Vec<usize> = frames
            .iter()
            .map(|f| {
                f.lines
                    .iter()
                    .map(|l| console::measure_text_width(l))
                    .max()
                    .unwrap_or(0)
            })
            .collect();

        let mut result = Frame::new();
        for row in 0..max_height {
            let mut line = String::new();
            for (col, f) in frames.iter().enumerate() {
                if col > 0 {
                    line.push_str(&gap_str);
                }
                if row < f.height() {
                    line.push_str(&f.lines[row]);
                    let visible = console::measure_text_width(&f.lines[row]);
                    if col < frames.len() - 1 && visible < widths[col] {
                        line.push_str(&" ".repeat(widths[col] - visible));
                    }
                } else if col < frames.len() - 1 {
                    line.push_str(&" ".repeat(widths[col]));
                }
            }
            result.push_line(line);
        }
        result
    }
}

/// Implement Render for Frame itself (pass-through).
impl Render for Frame {
    fn render(&self, _theme: &Theme) -> Frame {
        self.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::Text;

    #[test]
    fn vstack_composes() {
        let theme = Theme::plain();
        let layout = VStack::new()
            .child(Text::new("a"))
            .child(Text::new("b"));
        let frame = layout.render(&theme);
        assert_eq!(frame.height(), 2);
    }

    #[test]
    fn hstack_side_by_side() {
        let theme = Theme::plain();
        let layout = HStack::new()
            .gap(2)
            .child(Text::new("left"))
            .child(Text::new("right"));
        let frame = layout.render(&theme);
        assert_eq!(frame.height(), 1);
        assert!(frame.lines[0].contains("left"));
        assert!(frame.lines[0].contains("right"));
    }
}
