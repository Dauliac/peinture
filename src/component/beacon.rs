//! Beacon — composite component: tree + header bar.
//!
//! The beacon is a bottom-rooted tree of status items
//! anchored by a header line with a pulsing bar + brand text.
//!
//! ```rust,ignore
//! let beacon = Beacon::new(state)
//!     .animated(start_instant);  // or .static_display()
//!
//! let frame = beacon.render(&theme);
//! ```

use crate::component::bar::Bar;
use crate::component::frame::Frame;
use crate::component::text::Text;
use crate::component::tree::{Tree, TreeItem};
use crate::tokens::icons::StatusIcon;
use crate::tokens::{Semantic, Theme};
use crate::traits::{Animate, Render};
use std::time::Instant;

/// Severity level — drives the bar color when not animating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Ok,
    Warning,
    Error,
}

/// A single item in the beacon tree.
#[derive(Debug, Clone)]
pub struct BeaconItem {
    pub status: StatusIcon,
    pub message: String,
    pub metadata: Option<String>,
    pub detail: Option<String>,
    pub priority: u8,
}

/// State for the beacon (the ViewModel).
#[derive(Debug, Clone)]
pub struct BeaconState {
    pub brand: String,
    pub phase: Option<String>,
    pub progress: Option<String>,
    pub elapsed: Option<String>,
    pub items: Vec<BeaconItem>,
    pub severity: Severity,
    pub is_active: bool,
}

impl Default for BeaconState {
    fn default() -> Self {
        Self {
            brand: "cimera".into(),
            phase: None,
            progress: None,
            elapsed: None,
            items: Vec::new(),
            severity: Severity::Ok,
            is_active: false,
        }
    }
}

impl BeaconState {
    /// Sort items by priority (highest first) and truncate to max.
    pub fn visible_items(&self, max: usize) -> Vec<&BeaconItem> {
        let mut sorted: Vec<&BeaconItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted.truncate(max);
        sorted
    }
}

/// The Beacon component.
pub struct Beacon {
    state: BeaconState,
    bar: Bar,
}

impl Beacon {
    /// Create an animated beacon (pulse running).
    pub fn animated(state: BeaconState, start: Instant, theme: &Theme) -> Self {
        let bar = Bar::from_theme(theme)
            .start_at(start)
            .active(state.is_active);
        Self { state, bar }
    }

    /// Create a static beacon (no pulse).
    pub fn static_display(state: BeaconState, theme: &Theme) -> Self {
        let color = bar_semantic(state.severity);
        let bar = Bar::static_bar(theme.beacon.home_frame(), color);
        Self { state, bar }
    }

    /// Whether the pulse is at a rest position (safe to stop).
    pub fn is_at_rest(&self) -> bool {
        self.bar.is_at_rest()
    }
}

impl Render for Beacon {
    fn render(&self, theme: &Theme) -> Frame {
        let max_items = theme.beacon.max_items;
        let mut frame = Frame::new();

        // Tree items (bottom-rooted: grow upward from brand line)
        let visible = self.state.visible_items(max_items);
        let tree_items: Vec<TreeItem> = visible
            .into_iter()
            .map(|item| {
                let mut ti = TreeItem::new(item.status, &item.message);
                if let Some(ref m) = item.metadata { ti = ti.meta(m); }
                if let Some(ref d) = item.detail { ti = ti.detail(d); }
                ti
            })
            .collect();

        let tree = Tree::bottom_rooted().items(tree_items);
        frame.extend(&tree.render(theme));

        // Header line: bar + brand + phase + progress + elapsed
        let header = self.render_header(theme);
        frame.push_line(header);

        frame
    }
}

impl Beacon {
    fn render_header(&self, theme: &Theme) -> String {
        let mut parts = Vec::new();

        // Bar (animated or static)
        let bar_frame = self.bar.render(theme);
        parts.push(bar_frame.lines[0].clone());

        // Rainbow brand
        let brand = Text::rainbow(&self.state.brand);
        parts.push(brand.render(theme).lines[0].clone());

        // Phase icon + label
        if let Some(ref phase) = self.state.phase {
            let icon_semantic = if self.state.is_active {
                Semantic::Warning
            } else if self.state.severity == Severity::Error {
                Semantic::Error
            } else {
                Semantic::Success
            };
            let icon_status = if self.state.is_active {
                StatusIcon::InProgress
            } else if self.state.severity == Severity::Error {
                StatusIcon::Failed
            } else {
                StatusIcon::Success
            };

            let icon = theme.icons.for_status(icon_status);
            let color = icon_semantic.resolve(&theme.palette);
            parts.push(format!("{}{}\x1b[0m {}", color.fg_code(), icon, phase));
        }

        // Progress + elapsed (dim)
        if let Some(ref progress) = self.state.progress {
            parts.push(format!("\x1b[2m{}\x1b[0m", progress));
        }
        if let Some(ref elapsed) = self.state.elapsed {
            parts.push(format!("\x1b[2m{}\x1b[0m", elapsed));
        }

        parts.join("  ")
    }
}

fn bar_semantic(severity: Severity) -> Semantic {
    match severity {
        Severity::Ok => Semantic::BarIdle,
        Severity::Warning => Semantic::BarWarning,
        Severity::Error => Semantic::BarError,
    }
}

/// Convenience: render a live beacon.
pub fn render_live(state: &BeaconState, start: Instant, theme: &Theme) -> Frame {
    let beacon = Beacon::animated(state.clone(), start, theme);
    beacon.render(theme)
}

/// Convenience: render a static beacon.
pub fn render_static(state: &BeaconState, theme: &Theme) -> Frame {
    let beacon = Beacon::static_display(state.clone(), theme);
    beacon.render(theme)
}

/// Convenience: render CI mode (plain text, prefixed).
pub fn render_ci(state: &BeaconState, prefix: &str) -> Vec<String> {
    let mut lines = Vec::new();
    for item in &state.items {
        let icon = match item.status {
            StatusIcon::Success => "+",
            StatusIcon::Failed => "x",
            StatusIcon::InProgress => "*",
            StatusIcon::Cached => "=",
            _ => "-",
        };
        let meta = item.metadata.as_deref().unwrap_or("");
        lines.push(format!("[{prefix}] {icon} {} {meta}", item.message).trim_end().to_string());
    }
    if let Some(ref phase) = state.phase {
        let progress = state.progress.as_deref().unwrap_or("");
        let elapsed = state.elapsed.as_deref().unwrap_or("");
        lines.push(format!("[{prefix}] {phase} {progress} {elapsed}").trim().to_string());
    }
    lines
}

/// Check if a live beacon is at rest (safe to stop).
pub fn is_at_rest(start: Instant, theme: &Theme) -> bool {
    let bar = Bar::from_theme(theme).start_at(start);
    bar.is_at_rest()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_beacon_renders() {
        let state = BeaconState {
            phase: Some("Done".into()),
            items: vec![BeaconItem {
                status: StatusIcon::Success,
                message: "task".into(),
                metadata: Some("1s".into()),
                detail: None,
                priority: 10,
            }],
            ..BeaconState::default()
        };
        let theme = Theme::default();
        let frame = render_static(&state, &theme);
        assert!(frame.height() >= 2);
    }

    #[test]
    fn ci_mode_plain() {
        let state = BeaconState {
            phase: Some("Building...".into()),
            items: vec![BeaconItem {
                status: StatusIcon::Success,
                message: "done".into(),
                metadata: None,
                detail: None,
                priority: 10,
            }],
            ..BeaconState::default()
        };
        let lines = render_ci(&state, "cimera");
        assert!(lines.last().expect("lines").contains("Building"));
        assert!(!lines[0].contains('\x1b'));
    }

    #[test]
    fn priority_sorting() {
        let state = BeaconState {
            items: (0..10)
                .map(|i| BeaconItem {
                    status: StatusIcon::Success,
                    message: format!("item {i}"),
                    metadata: None,
                    detail: None,
                    priority: i,
                })
                .collect(),
            ..BeaconState::default()
        };
        let visible = state.visible_items(5);
        assert_eq!(visible.len(), 5);
        assert_eq!(visible[0].priority, 9);
    }
}
