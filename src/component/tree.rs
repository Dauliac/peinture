//! Tree component — hierarchical item list with connectors.
//!
//! ```rust,ignore
//! let tree = Tree::bottom_rooted()
//!     .items(vec![
//!         TreeItem::new(StatusIcon::Success, "task done").meta("1.2s"),
//!         TreeItem::new(StatusIcon::Failed, "build failed").detail("type error"),
//!     ]);
//! ```

use crate::component::Frame;
use crate::tokens::icons::StatusIcon;
use crate::tokens::{Semantic, Theme};
use crate::traits::Render;

/// A single item in a tree.
#[derive(Debug, Clone)]
pub struct TreeItem {
    pub status: StatusIcon,
    pub message: String,
    pub metadata: Option<String>,
    pub detail: Option<String>,
    /// Override the color (ignores status-based color). Used for workload items.
    pub color_override: Option<Semantic>,
    /// Fade factor: 0.0 = full color, 1.0 = fully dim. Used for aging notifications.
    pub fade_factor: f32,
}

impl TreeItem {
    /// Create a tree item.
    pub fn new(status: StatusIcon, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            metadata: None,
            detail: None,
            color_override: None,
            fade_factor: 0.0,
        }
    }

    /// Add right-aligned metadata (duration, count, etc.).
    pub fn meta(mut self, meta: impl Into<String>) -> Self {
        self.metadata = Some(meta.into());
        self
    }

    /// Add a detail line below the message (for errors, hints).
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    /// Override the color for this item (e.g., yellow for workload).
    pub fn color_override(mut self, semantic: Semantic) -> Self {
        self.color_override = Some(semantic);
        self
    }

    /// Set the fade factor (0.0 = full color, 1.0 = fully dim).
    pub fn fade(mut self, fade: f32) -> Self {
        self.fade_factor = fade;
        self
    }
}

/// Tree layout direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeRoot {
    /// Root at top, last item gets └─ (traditional).
    Top,
    /// Root at bottom, first item gets ┌─ (beacon style).
    Bottom,
}

/// Tree component.
#[derive(Debug, Clone)]
pub struct Tree {
    items: Vec<TreeItem>,
    root: TreeRoot,
    max_items: Option<usize>,
}

impl Tree {
    /// Create a tree with the given root direction.
    pub fn new(root: TreeRoot) -> Self {
        Self { items: Vec::new(), root, max_items: None }
    }

    /// Top-rooted tree (traditional: root at top, last item gets └─).
    pub fn top_rooted() -> Self {
        Self { items: Vec::new(), root: TreeRoot::Top, max_items: None }
    }

    /// Bottom-rooted tree (beacon style: root at bottom, first item gets ┌─).
    pub fn bottom_rooted() -> Self {
        Self { items: Vec::new(), root: TreeRoot::Bottom, max_items: None }
    }

    /// Set items.
    pub fn items(mut self, items: Vec<TreeItem>) -> Self {
        self.items = items;
        self
    }

    /// Limit the number of displayed items.
    pub fn max_items(mut self, max: usize) -> Self {
        self.max_items = Some(max);
        self
    }
}

impl Render for Tree {
    fn render(&self, theme: &Theme) -> Frame {
        let items = if let Some(max) = self.max_items {
            &self.items[..self.items.len().min(max)]
        } else {
            &self.items
        };

        let mut frame = Frame::new();

        for (i, item) in items.iter().enumerate() {
            let connector = match self.root {
                TreeRoot::Bottom => theme.tree.connector_bottom_rooted(i == 0),
                TreeRoot::Top => theme.tree.connector(i == items.len() - 1),
            };
            let continuation = match self.root {
                TreeRoot::Bottom => theme.tree.continuation_bottom_rooted(i == 0),
                TreeRoot::Top => theme.tree.continuation(i == items.len() - 1),
            };

            let icon = theme.icons.for_status(item.status);
            let semantic = item.color_override.unwrap_or_else(|| semantic_for_status(item.status));
            let base_color = semantic.resolve(&theme.palette);

            // 3-stage fade: full color → dim → dark grey → removed
            // fade 0.0–0.5: dim attribute (terminal's native dimming)
            // fade 0.5–1.0: dark grey, NO dim (color speaks for itself)
            let fade = item.fade_factor;
            let (dim_prefix, icon_color) = if fade > 0.5 {
                // Dark grey phase (last half) — no dim, just grey color
                let grey = crate::tokens::Color::Rgb(70, 70, 70);
                ("", grey)
            } else if fade > 0.0 {
                // Dim phase (first half)
                ("\x1b[2m", base_color)
            } else {
                ("", base_color)
            };
            let msg_color = icon_color;

            let main = if let Some(ref meta) = item.metadata {
                let prefix = format!(
                    "{dim_prefix}\x1b[2m{connector}\x1b[0m {dim_prefix}{ic}{icon}\x1b[0m {dim_prefix}{mc}{msg}\x1b[0m",
                    ic = icon_color.fg_code(),
                    mc = msg_color.fg_code(),
                    msg = item.message,
                );
                let visible_len = console::measure_text_width(&strip_ansi(&prefix));
                let pad = if visible_len < theme.spacing.metadata_column {
                    " ".repeat(theme.spacing.metadata_column - visible_len)
                } else {
                    " ".into()
                };
                format!("{prefix}{pad}{dim_prefix}\x1b[2m({meta})\x1b[0m")
            } else {
                format!(
                    "{dim_prefix}\x1b[2m{connector}\x1b[0m {dim_prefix}{ic}{icon}\x1b[0m {dim_prefix}{mc}{msg}\x1b[0m",
                    ic = icon_color.fg_code(),
                    mc = msg_color.fg_code(),
                    msg = item.message,
                )
            };
            frame.push_line(main);

            if let Some(ref detail) = item.detail {
                frame.push_line(format!(
                    "{dim_prefix}\x1b[2m{continuation}\x1b[0m    {dim_prefix}\x1b[2m{detail}\x1b[0m"
                ));
            }
        }

        frame
    }
}

fn semantic_for_status(status: StatusIcon) -> Semantic {
    match status {
        StatusIcon::Success => Semantic::Success,
        StatusIcon::Failed => Semantic::Error,
        StatusIcon::InProgress | StatusIcon::Warning => Semantic::Warning,
        StatusIcon::Cached | StatusIcon::Info => Semantic::Info,
        StatusIcon::Pending | StatusIcon::Skipped => Semantic::Muted,
    }
}

fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if ch == 'm' { in_escape = false; }
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_tree() {
        let tree = Tree::bottom_rooted();
        let theme = Theme::default();
        assert!(tree.render(&theme).is_empty());
    }

    #[test]
    fn single_item() {
        let tree = Tree::bottom_rooted().items(vec![
            TreeItem::new(StatusIcon::Success, "done").meta("1.2s"),
        ]);
        let theme = Theme::default();
        let frame = tree.render(&theme);
        assert_eq!(frame.height(), 1);
        assert!(strip_ansi(&frame.lines[0]).contains("done"));
    }

    #[test]
    fn bottom_rooted_first_gets_corner() {
        let tree = Tree::bottom_rooted().items(vec![
            TreeItem::new(StatusIcon::Success, "first"),
            TreeItem::new(StatusIcon::Success, "second"),
        ]);
        let theme = Theme::default();
        let frame = tree.render(&theme);
        assert!(frame.lines[0].contains("\u{250C}")); // ┌
        assert!(frame.lines[1].contains("\u{251C}")); // ├
    }

    #[test]
    fn detail_adds_line() {
        let tree = Tree::top_rooted().items(vec![
            TreeItem::new(StatusIcon::Failed, "err").detail("cause"),
        ]);
        let theme = Theme::default();
        let frame = tree.render(&theme);
        assert_eq!(frame.height(), 2);
    }
}
