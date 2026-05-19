//! Frame — the output of rendering a component.

/// A rendered frame — a list of styled lines ready for the painter.
///
/// Frames are the universal output type. Components produce them,
/// layout containers compose them, and the painter draws them.
#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub lines: Vec<String>,
}

impl Frame {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Create a frame from a single line.
    pub fn line(line: impl Into<String>) -> Self {
        Self {
            lines: vec![line.into()],
        }
    }

    /// Create a frame from multiple lines.
    pub fn from_lines(lines: impl IntoIterator<Item = String>) -> Self {
        Self {
            lines: lines.into_iter().collect(),
        }
    }

    /// Append a line to this frame.
    pub fn push_line(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }

    /// Append all lines from another frame.
    pub fn extend(&mut self, other: &Frame) {
        self.lines.extend(other.lines.iter().cloned());
    }

    /// Number of lines.
    pub fn height(&self) -> usize {
        self.lines.len()
    }

    /// Whether the frame has no lines.
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}
