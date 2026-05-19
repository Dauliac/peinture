//! Beacon — composite component with two layout orientations.
//!
//! The beacon has two **slots** (like Vue/React):
//! - **Header**: bar + brand text + phase + metadata
//! - **Body**: tree of status items
//!
//! The `Orientation` enum controls how these slots are composed:
//!
//! ```text
//! BottomUp (execution, pinned):       TopDown (notifications, under prompt):
//!
//! ┌─ ✓ eval completed     (2.1s)      ▌ cimera  ✓ Done  6.3s
//! ├─ ✓ myservice:rust      (4.2s)      ├─ ✓ eval completed       (6.2s)
//! ├─ ● myservice:go        (cached)    ├─ ✓ registry refreshed   (102 tasks)
//! ▌ cimera  ✓ Done  6.3s              └─ ✓ hooks: rust, aws
//! ```
//!
//! Same data, same header, same tree. Only composition order and
//! tree connector style change.
//!
//! ```rust,ignore
//! // Execution mode (bottom-up, default)
//! let beacon = Beacon::animated(state, start, &theme);
//!
//! // Notification mode (top-down)
//! let beacon = Beacon::static_display(state, &theme)
//!     .orientation(Orientation::TopDown);
//! ```

use crate::component::bar::Bar;
use crate::component::frame::Frame;
use crate::component::text::Text;
use crate::component::tree::{Tree, TreeItem, TreeRoot};
use crate::tokens::icons::StatusIcon;
use crate::tokens::{Semantic, Theme};
use crate::traits::{Animate, Render};
use std::time::Instant;

// ─────────────────────────────────────────────────────────────────────────
// Orientation
// ─────────────────────────────────────────────────────────────────────────

/// Layout direction for the beacon.
///
/// Controls composition order of header and tree, and tree connector style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    /// Header at bottom, tree grows upward. First item gets `┌─`.
    /// Used for execution mode (pinned at terminal bottom).
    #[default]
    BottomUp,
    /// Header at top, tree grows downward. Last item gets `└─`.
    /// Used for notifications (printed under shell prompt).
    TopDown,
}

impl Orientation {
    /// Map orientation to tree root direction.
    fn tree_root(self) -> TreeRoot {
        match self {
            Self::BottomUp => TreeRoot::Bottom,
            Self::TopDown => TreeRoot::Top,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Data (ViewModel)
// ─────────────────────────────────────────────────────────────────────────

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

/// State for the beacon (the ViewModel — pure data, no display logic).
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

// ─────────────────────────────────────────────────────────────────────────
// Component
// ─────────────────────────────────────────────────────────────────────────

/// The Beacon component — composes a header and tree with configurable orientation.
pub struct Beacon {
    state: BeaconState,
    bar: Bar,
    orientation: Orientation,
}

impl Beacon {
    /// Create an animated beacon (pulse running). Default: `BottomUp`.
    pub fn animated(state: BeaconState, start: Instant, theme: &Theme) -> Self {
        let bar = Bar::from_theme(theme)
            .start_at(start)
            .active(state.is_active);
        Self { state, bar, orientation: Orientation::default() }
    }

    /// Create a static beacon (no pulse). Default: `BottomUp`.
    pub fn static_display(state: BeaconState, theme: &Theme) -> Self {
        let color = bar_semantic(state.severity);
        let bar = Bar::static_bar(theme.beacon.home_frame(), color);
        Self { state, bar, orientation: Orientation::default() }
    }

    /// Set the orientation. Builder pattern for chaining.
    pub fn orientation(mut self, orientation: Orientation) -> Self {
        self.orientation = orientation;
        self
    }

    /// Whether the pulse is at a rest position (safe to stop).
    pub fn is_at_rest(&self) -> bool {
        self.bar.is_at_rest()
    }
}

impl Render for Beacon {
    fn render(&self, theme: &Theme) -> Frame {
        let header = self.render_header(theme);
        let tree_frame = self.render_tree(theme);

        let mut frame = Frame::new();
        match self.orientation {
            Orientation::BottomUp => {
                // Tree first (grows upward), then header at the bottom
                frame.extend(&tree_frame);
                frame.push_line(header);
            }
            Orientation::TopDown => {
                // Header first (at the top), then tree grows downward
                frame.push_line(header);
                frame.extend(&tree_frame);
            }
        }
        frame
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Internal rendering (shared between orientations)
// ─────────────────────────────────────────────────────────────────────────

impl Beacon {
    /// Render the tree slot — items with connectors.
    /// Connector style is determined by orientation.
    fn render_tree(&self, theme: &Theme) -> Frame {
        let max_items = theme.beacon.max_items;
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

        let tree = Tree::new(self.orientation.tree_root()).items(tree_items);
        tree.render(theme)
    }

    /// Render the header slot — bar + brand + phase + metadata.
    /// Identical for both orientations.
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
            let (icon_semantic, icon_status) = if self.state.is_active {
                (Semantic::Warning, StatusIcon::InProgress)
            } else if self.state.severity == Severity::Error {
                (Semantic::Error, StatusIcon::Failed)
            } else {
                (Semantic::Success, StatusIcon::Success)
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

// ─────────────────────────────────────────────────────────────────────────
// Convenience functions
// ─────────────────────────────────────────────────────────────────────────

/// Render a live beacon (animated, bottom-up by default).
pub fn render_live(state: &BeaconState, start: Instant, theme: &Theme) -> Frame {
    Beacon::animated(state.clone(), start, theme).render(theme)
}

/// Render a live beacon with explicit orientation.
pub fn render_live_oriented(
    state: &BeaconState,
    start: Instant,
    theme: &Theme,
    orientation: Orientation,
) -> Frame {
    Beacon::animated(state.clone(), start, theme)
        .orientation(orientation)
        .render(theme)
}

/// Render a static beacon (no pulse, bottom-up by default).
pub fn render_static(state: &BeaconState, theme: &Theme) -> Frame {
    Beacon::static_display(state.clone(), theme).render(theme)
}

/// Render a static beacon with explicit orientation.
pub fn render_static_oriented(
    state: &BeaconState,
    theme: &Theme,
    orientation: Orientation,
) -> Frame {
    Beacon::static_display(state.clone(), theme)
        .orientation(orientation)
        .render(theme)
}

/// Render CI mode (plain text, prefixed). Orientation doesn't apply.
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

// ─────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_state() -> BeaconState {
        BeaconState {
            phase: Some("Done".into()),
            items: vec![
                BeaconItem {
                    status: StatusIcon::Success,
                    message: "first".into(),
                    metadata: Some("1s".into()),
                    detail: None,
                    priority: 10,
                },
                BeaconItem {
                    status: StatusIcon::Cached,
                    message: "second".into(),
                    metadata: Some("cached".into()),
                    detail: None,
                    priority: 5,
                },
            ],
            ..BeaconState::default()
        }
    }

    #[test]
    fn bottom_up_header_is_last_line() {
        let state = sample_state();
        let theme = Theme::default();
        let frame = Beacon::static_display(state, &theme)
            .orientation(Orientation::BottomUp)
            .render(&theme);
        // Header (with "cimera") should be the last line
        let last = frame.lines.last().expect("has lines");
        assert!(last.contains("cimera") || last.contains("Done"));
    }

    #[test]
    fn top_down_header_is_first_line() {
        let state = sample_state();
        let theme = Theme::default();
        let frame = Beacon::static_display(state, &theme)
            .orientation(Orientation::TopDown)
            .render(&theme);
        // Header (with "cimera") should be the first line
        let first = &frame.lines[0];
        assert!(first.contains("cimera") || first.contains("Done"));
    }

    #[test]
    fn both_orientations_same_height() {
        let state = sample_state();
        let theme = Theme::default();
        let bottom_up = Beacon::static_display(state.clone(), &theme)
            .orientation(Orientation::BottomUp)
            .render(&theme);
        let top_down = Beacon::static_display(state, &theme)
            .orientation(Orientation::TopDown)
            .render(&theme);
        assert_eq!(bottom_up.height(), top_down.height());
    }

    #[test]
    fn bottom_up_tree_has_top_corner() {
        let state = sample_state();
        let theme = Theme::default();
        let frame = Beacon::static_display(state, &theme)
            .orientation(Orientation::BottomUp)
            .render(&theme);
        // First line should have ┌ (top corner for bottom-rooted tree)
        assert!(frame.lines[0].contains('\u{250C}'));
    }

    #[test]
    fn top_down_tree_has_bottom_corner() {
        let state = sample_state();
        let theme = Theme::default();
        let frame = Beacon::static_display(state, &theme)
            .orientation(Orientation::TopDown)
            .render(&theme);
        // Last line should have └ (bottom corner for top-rooted tree)
        let last = frame.lines.last().expect("has lines");
        assert!(last.contains('\u{2514}'));
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

    #[test]
    fn default_orientation_is_bottom_up() {
        assert_eq!(Orientation::default(), Orientation::BottomUp);
    }
}
