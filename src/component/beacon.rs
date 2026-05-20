//! Beacon — composite component with two layout orientations.
//!
//! The beacon tree has two kinds of items:
//! - **Notifications**: scroll from bottom to top, max 4 kept, oldest drops off
//! - **Workload**: the current active task, always at the bottom of the tree (closest to brand)
//!
//! ```text
//! BottomUp (execution):               TopDown (notifications):
//!
//! ┌─ ✓ eval completed     (2.1s)      █ cimera
//! ├─ ✓ registry refreshed  (102)       ├─ ✓ eval completed       (6.2s)
//! ├─ ✗ aws: SSO expired               ├─ ✓ registry refreshed   (102)
//! ├─ ◐ building myservice  (12.3s)     ├─ ✗ aws: SSO expired
//! █ cimera  ◐ Building...              └─ ◐ building myservice   (12.3s)
//! ```

use crate::component::bar::Bar;
use crate::component::frame::Frame;
use crate::component::text::Text;
use crate::component::tree::{Tree, TreeItem, TreeRoot};
use crate::tokens::icons::StatusIcon;
use crate::tokens::{Semantic, Theme};
use crate::traits::{Animate, Render};
use std::time::{Duration, Instant};

// ─────────────────────────────────────────────────────────────────────────
// Orientation
// ─────────────────────────────────────────────────────────────────────────

/// Layout direction for the beacon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientation {
    /// Header at bottom, tree grows upward.
    #[default]
    BottomUp,
    /// Header at top, tree grows downward.
    TopDown,
}

impl Orientation {
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

/// Kind of beacon item — determines rendering behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemKind {
    /// Notification: scrolls, max 4 kept, oldest drops off the top.
    /// Colors: success=green, warning=orange, error=red.
    Notification,
    /// Current workload: always at the bottom of the tree (closest to brand).
    /// Always yellow regardless of status.
    Workload,
}

/// A single item in the beacon tree.
#[derive(Debug, Clone)]
pub struct BeaconItem {
    pub status: StatusIcon,
    pub message: String,
    pub metadata: Option<String>,
    pub detail: Option<String>,
    pub kind: ItemKind,
    /// When this item was created (for TTL/fade on notifications).
    pub created_at: Instant,
}

impl BeaconItem {
    /// Create a notification item.
    pub fn notification(status: StatusIcon, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            metadata: None,
            detail: None,
            kind: ItemKind::Notification,
            created_at: Instant::now(),
        }
    }

    /// Create a workload item (current active task).
    pub fn workload(status: StatusIcon, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            metadata: None,
            detail: None,
            kind: ItemKind::Workload,
            created_at: Instant::now(),
        }
    }

    /// Age of this item since creation.
    pub fn age(&self) -> Duration {
        self.created_at.elapsed()
    }

    /// Fade factor: 0.0 = full color, 1.0 = fully faded.
    /// Returns None for workload items (they never fade).
    pub fn fade(&self, ttl_ms: u64, fade_start: f32) -> f32 {
        if self.kind == ItemKind::Workload {
            return 0.0;
        }
        let age_ms = self.age().as_millis() as f64;
        let ttl = ttl_ms as f64;
        if age_ms >= ttl {
            return 1.0;
        }
        let fade_begin = ttl * fade_start as f64;
        if age_ms <= fade_begin {
            return 0.0;
        }
        // Linear fade from fade_start to TTL
        ((age_ms - fade_begin) / (ttl - fade_begin)) as f32
    }

    /// Add right-aligned metadata.
    pub fn meta(mut self, meta: impl Into<String>) -> Self {
        self.metadata = Some(meta.into());
        self
    }

    /// Add a detail line below the message.
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Maximum number of notifications kept (oldest scroll off).
pub const MAX_NOTIFICATIONS: usize = 4;

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
    /// Push a notification. Keeps only the last MAX_NOTIFICATIONS.
    pub fn push_notification(&mut self, item: BeaconItem) {
        self.items.push(item);
        self.trim_notifications();
    }

    /// Remove notifications that have exceeded their TTL.
    pub fn gc_expired(&mut self, ttl_ms: u64) {
        self.items.retain(|item| {
            if item.kind != ItemKind::Notification {
                return true; // keep workload items
            }
            (item.age().as_millis() as u64) < ttl_ms
        });
    }

    /// Set or replace the current workload item.
    pub fn set_workload(&mut self, item: BeaconItem) {
        // Remove any existing workload
        self.items.retain(|i| i.kind != ItemKind::Workload);
        self.items.push(item);
    }

    /// Clear the workload (task finished).
    pub fn clear_workload(&mut self) {
        self.items.retain(|i| i.kind != ItemKind::Workload);
    }

    /// Trim notifications to MAX_NOTIFICATIONS (oldest first).
    fn trim_notifications(&mut self) {
        let notif_count = self.items.iter().filter(|i| i.kind == ItemKind::Notification).count();
        if notif_count > MAX_NOTIFICATIONS {
            let to_remove = notif_count - MAX_NOTIFICATIONS;
            let mut removed = 0;
            self.items.retain(|i| {
                if i.kind == ItemKind::Notification && removed < to_remove {
                    removed += 1;
                    false // drop oldest
                } else {
                    true
                }
            });
        }
    }

    /// Get the ordered items for rendering:
    /// notifications first (in order), workload last (closest to brand).
    fn render_items(&self) -> Vec<&BeaconItem> {
        let mut notifications: Vec<&BeaconItem> = self.items.iter()
            .filter(|i| i.kind == ItemKind::Notification)
            .collect();
        let workload: Vec<&BeaconItem> = self.items.iter()
            .filter(|i| i.kind == ItemKind::Workload)
            .collect();

        // Notifications on top, workload at bottom (closest to brand line)
        notifications.extend(workload);
        notifications
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Component
// ─────────────────────────────────────────────────────────────────────────

/// The Beacon component.
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

    /// Set the orientation.
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

        // No padding — render only actual content.
        // The Painter handles fixed-height reservation separately.
        let mut frame = Frame::new();
        match self.orientation {
            Orientation::BottomUp => {
                frame.extend(&tree_frame);
                frame.push_line(header);
            }
            Orientation::TopDown => {
                frame.push_line(header);
                frame.extend(&tree_frame);
            }
        }
        frame
    }

    /// The maximum height this beacon can occupy.
    /// The Painter uses this to set a fixed scroll region.
    fn height(&self, theme: &Theme) -> usize {
        theme.beacon.max_items + 1
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Internal rendering
// ─────────────────────────────────────────────────────────────────────────

impl Beacon {
    fn render_tree(&self, theme: &Theme) -> Frame {
        let ordered = self.state.render_items();
        let ttl_ms = theme.beacon.notification_ttl_ms;
        let fade_start = theme.beacon.notification_fade_start;

        let tree_items: Vec<TreeItem> = ordered
            .into_iter()
            .map(|item| {
                let mut ti = TreeItem::new(item.status, &item.message);
                if let Some(ref m) = item.metadata { ti = ti.meta(m); }
                if let Some(ref d) = item.detail { ti = ti.detail(d); }
                if item.kind == ItemKind::Workload {
                    ti = ti.color_override(Semantic::Warning);
                }
                // Apply fade for aging notifications
                let f = item.fade(ttl_ms, fade_start);
                if f > 0.0 {
                    ti = ti.fade(f);
                }
                ti
            })
            .collect();

        let tree = Tree::new(self.orientation.tree_root()).items(tree_items);
        tree.render(theme)
    }

    fn render_header(&self, theme: &Theme) -> String {
        let mut header = String::new();

        let bar_frame = self.bar.render(theme);
        header.push_str(&bar_frame.lines[0]);

        let brand = Text::rainbow(&self.state.brand);
        header.push_str(&brand.render(theme).lines[0]);

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
            header.push_str(&format!("  {}{}\x1b[0m {}", color.fg_code(), icon, phase));
        }

        if let Some(ref progress) = self.state.progress {
            header.push_str(&format!("  \x1b[2m{}\x1b[0m", progress));
        }
        if let Some(ref elapsed) = self.state.elapsed {
            header.push_str(&format!("  \x1b[2m{}\x1b[0m", elapsed));
        }

        header
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

pub fn render_live(state: &BeaconState, start: Instant, theme: &Theme) -> Frame {
    let mut s = state.clone();
    s.gc_expired(theme.beacon.notification_ttl_ms);
    Beacon::animated(s, start, theme).render(theme)
}

pub fn render_live_oriented(state: &BeaconState, start: Instant, theme: &Theme, orientation: Orientation) -> Frame {
    Beacon::animated(state.clone(), start, theme).orientation(orientation).render(theme)
}

pub fn render_static(state: &BeaconState, theme: &Theme) -> Frame {
    Beacon::static_display(state.clone(), theme).render(theme)
}

pub fn render_static_oriented(state: &BeaconState, theme: &Theme, orientation: Orientation) -> Frame {
    Beacon::static_display(state.clone(), theme).orientation(orientation).render(theme)
}

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

    #[test]
    fn notifications_capped_at_max() {
        let mut state = BeaconState::default();
        for i in 0..10 {
            state.push_notification(BeaconItem::notification(StatusIcon::Success, format!("item {i}")));
        }
        let notifs: Vec<_> = state.items.iter().filter(|i| i.kind == ItemKind::Notification).collect();
        assert_eq!(notifs.len(), MAX_NOTIFICATIONS);
        // Oldest dropped, newest kept
        assert!(notifs.last().expect("has items").message.contains("item 9"));
    }

    #[test]
    fn workload_always_last_in_render() {
        let mut state = BeaconState::default();
        state.set_workload(BeaconItem::workload(StatusIcon::InProgress, "building..."));
        state.push_notification(BeaconItem::notification(StatusIcon::Success, "eval done"));
        state.push_notification(BeaconItem::notification(StatusIcon::Success, "registry ok"));

        let ordered = state.render_items();
        // Workload should be last
        assert_eq!(ordered.last().expect("has items").kind, ItemKind::Workload);
        // Notifications should come first
        assert_eq!(ordered[0].kind, ItemKind::Notification);
    }

    #[test]
    fn set_workload_replaces_existing() {
        let mut state = BeaconState::default();
        state.set_workload(BeaconItem::workload(StatusIcon::InProgress, "old task"));
        state.set_workload(BeaconItem::workload(StatusIcon::InProgress, "new task"));
        let workloads: Vec<_> = state.items.iter().filter(|i| i.kind == ItemKind::Workload).collect();
        assert_eq!(workloads.len(), 1);
        assert_eq!(workloads[0].message, "new task");
    }

    #[test]
    fn clear_workload_removes_it() {
        let mut state = BeaconState::default();
        state.set_workload(BeaconItem::workload(StatusIcon::InProgress, "task"));
        state.push_notification(BeaconItem::notification(StatusIcon::Success, "notif"));
        state.clear_workload();
        assert!(state.items.iter().all(|i| i.kind == ItemKind::Notification));
    }

    #[test]
    fn bottom_up_header_last() {
        let mut state = BeaconState { phase: Some("Done".into()), ..BeaconState::default() };
        state.push_notification(BeaconItem::notification(StatusIcon::Success, "task"));
        let theme = Theme::default();
        let frame = Beacon::static_display(state, &theme).orientation(Orientation::BottomUp).render(&theme);
        let last = frame.lines.last().expect("has lines");
        assert!(last.contains("cimera") || last.contains("Done"));
    }

    #[test]
    fn top_down_header_first() {
        let mut state = BeaconState { phase: Some("Done".into()), ..BeaconState::default() };
        state.push_notification(BeaconItem::notification(StatusIcon::Success, "task"));
        let theme = Theme::default();
        let frame = Beacon::static_display(state, &theme).orientation(Orientation::TopDown).render(&theme);
        assert!(frame.lines[0].contains("cimera") || frame.lines[0].contains("Done"));
    }
}
