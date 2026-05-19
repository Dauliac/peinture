//! Tree renderer — hierarchical item list with connectors.

use crate::tokens::{Theme, icons::StatusIcon};

/// A single item in a tree.
#[derive(Debug, Clone)]
pub struct TreeItem {
    /// Status icon to display.
    pub status: StatusIcon,
    /// Main message text.
    pub message: String,
    /// Optional right-aligned metadata (duration, count, etc.).
    pub metadata: Option<String>,
    /// Optional detail line below the message (for errors, hints).
    pub detail: Option<String>,
}

/// Tree component — renders a list of items with connectors.
pub struct Tree;

impl Tree {
    /// Render items as a top-rooted tree (last item gets └─).
    pub fn render(items: &[TreeItem], theme: &Theme) -> Vec<String> {
        Self::render_inner(items, theme, false)
    }

    /// Render items as a bottom-rooted tree (first item gets ┌─, root is below).
    /// Used by the beacon where the brand line is the root at the bottom.
    pub fn render_bottom_rooted(items: &[TreeItem], theme: &Theme) -> Vec<String> {
        Self::render_inner(items, theme, true)
    }

    fn render_inner(items: &[TreeItem], theme: &Theme, bottom_rooted: bool) -> Vec<String> {
        let mut lines = Vec::new();
        let last_idx = items.len().saturating_sub(1);

        for (i, item) in items.iter().enumerate() {
            let connector = if bottom_rooted {
                theme.tree.connector_bottom_rooted(i == 0)
            } else {
                theme.tree.connector(i == last_idx)
            };

            let continuation = if bottom_rooted {
                theme.tree.continuation_bottom_rooted(i == 0)
            } else {
                theme.tree.continuation(i == last_idx)
            };

            // Status icon with color
            let icon = theme.icons.for_status(item.status);
            let icon_color = color_for_status(item.status, theme);
            let msg_color = color_for_status(item.status, theme);

            // Build main line
            let main = if let Some(ref meta) = item.metadata {
                let prefix = format!(
                    "{dim}{connector}{reset} {icon_fg}{icon}{reset} {msg_fg}{msg}{reset}",
                    dim = Theme::dim(),
                    reset = Theme::reset(),
                    icon_fg = icon_color,
                    msg_fg = msg_color,
                    msg = item.message,
                );
                let visible_len = console::measure_text_width(&strip_ansi(&prefix));
                let padding = if visible_len < theme.spacing.metadata_column {
                    " ".repeat(theme.spacing.metadata_column - visible_len)
                } else {
                    " ".into()
                };
                format!(
                    "{prefix}{padding}{dim}({meta}){reset}",
                    dim = Theme::dim(),
                    reset = Theme::reset(),
                    meta = meta,
                )
            } else {
                format!(
                    "{dim}{connector}{reset} {icon_fg}{icon}{reset} {msg_fg}{msg}{reset}",
                    dim = Theme::dim(),
                    reset = Theme::reset(),
                    icon_fg = icon_color,
                    msg_fg = msg_color,
                    msg = item.message,
                )
            };

            lines.push(main);

            // Detail line (indented under continuation)
            if let Some(ref detail) = item.detail {
                lines.push(format!(
                    "{dim}{continuation}{reset}    {dim}{detail}{reset}",
                    dim = Theme::dim(),
                    reset = Theme::reset(),
                ));
            }
        }

        lines
    }
}

fn color_for_status(status: StatusIcon, theme: &Theme) -> String {
    match status {
        StatusIcon::Success => theme.palette.success.fg_code(),
        StatusIcon::Failed => theme.palette.error.fg_code(),
        StatusIcon::InProgress | StatusIcon::Warning => theme.palette.warning.fg_code(),
        StatusIcon::Cached | StatusIcon::Info => theme.palette.info.fg_code(),
        StatusIcon::Pending | StatusIcon::Skipped => theme.palette.muted.fg_code(),
    }
}

/// Strip ANSI escape codes from a string (for width measurement).
fn strip_ansi(s: &str) -> String {
    let mut out = String::new();
    let mut in_escape = false;
    for ch in s.chars() {
        if ch == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if ch == 'm' {
                in_escape = false;
            }
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
    fn render_empty() {
        let theme = Theme::default();
        let lines = Tree::render(&[], &theme);
        assert!(lines.is_empty());
    }

    #[test]
    fn render_single_item() {
        let theme = Theme::default();
        let items = vec![TreeItem {
            status: StatusIcon::Success,
            message: "task completed".into(),
            metadata: Some("1.2s".into()),
            detail: None,
        }];
        let lines = Tree::render(&items, &theme);
        assert_eq!(lines.len(), 1);
        assert!(strip_ansi(&lines[0]).contains("task completed"));
        assert!(strip_ansi(&lines[0]).contains("1.2s"));
    }

    #[test]
    fn render_with_detail() {
        let theme = Theme::default();
        let items = vec![TreeItem {
            status: StatusIcon::Failed,
            message: "build failed".into(),
            metadata: None,
            detail: Some("type error in main.rs:42".into()),
        }];
        let lines = Tree::render(&items, &theme);
        assert_eq!(lines.len(), 2);
        assert!(strip_ansi(&lines[1]).contains("type error"));
    }

    #[test]
    fn strip_ansi_works() {
        assert_eq!(strip_ansi("\x1b[32mhello\x1b[0m"), "hello");
        assert_eq!(strip_ansi("plain"), "plain");
    }
}
