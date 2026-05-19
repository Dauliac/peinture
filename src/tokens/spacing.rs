//! Spacing tokens — indentation, margins, alignment.

use serde::Deserialize;

/// Spacing configuration for layout.
#[derive(Debug, Clone, Deserialize)]
pub struct Spacing {
    /// Characters per indent level (default: 2).
    pub indent_width: usize,
    /// Tree connector width including trailing space (default: 3, e.g. "|- ").
    pub tree_indent: usize,
    /// Target column for right-aligned metadata (default: 50).
    pub metadata_column: usize,
}

impl Default for Spacing {
    fn default() -> Self {
        Self {
            indent_width: 2,
            tree_indent: 3,
            metadata_column: 50,
        }
    }
}

impl Spacing {
    /// Generate N levels of indentation.
    pub fn indent(&self, level: usize) -> String {
        " ".repeat(self.indent_width * level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indent_levels() {
        let s = Spacing::default();
        assert_eq!(s.indent(0), "");
        assert_eq!(s.indent(1), "  ");
        assert_eq!(s.indent(2), "    ");
    }
}
