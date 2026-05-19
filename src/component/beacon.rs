//! Beacon component — pinned status panel with header, tree, and pulse.
//!
//! The beacon is the primary composite component: a header line with
//! a pulsing bar + rainbow brand text + phase info, followed by a
//! tree of status items.
//!
//! # Modes
//!
//! - **Live**: animated pulse, updating items (during execution)
//! - **Static**: no animation, fixed content (shell hook, completion)
//! - **CI**: prefixed lines, no tree chars, no animation

use crate::component::pulse::Pulse;
use crate::component::rainbow::Rainbow;
use crate::component::tree::{Tree, TreeItem};
use crate::component::Frame;
use crate::tokens::icons::StatusIcon;
use crate::tokens::Theme;

/// Severity level — drives the bar color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Ok,
    Warning,
    Error,
}

/// State for the beacon component.
#[derive(Debug, Clone)]
pub struct BeaconState {
    /// Brand name displayed in rainbow.
    pub brand: String,
    /// Current phase label (e.g., "Building...", "Done").
    pub phase: Option<String>,
    /// Progress counter (e.g., "2/5 tasks").
    pub progress: Option<String>,
    /// Elapsed time string (e.g., "12.3s").
    pub elapsed: Option<String>,
    /// Tree items (max beacon.max_items from theme).
    pub items: Vec<BeaconItem>,
    /// Overall severity (drives bar color).
    pub severity: Severity,
    /// Whether the beacon is actively doing work (drives pulse animation).
    pub is_active: bool,
}

/// A single item in the beacon tree.
#[derive(Debug, Clone)]
pub struct BeaconItem {
    pub status: StatusIcon,
    pub message: String,
    pub metadata: Option<String>,
    pub detail: Option<String>,
    /// Priority for sorting into limited slots (higher = more important).
    pub priority: u8,
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
    /// Sort and truncate items to fit max_items.
    pub fn visible_items(&self, max: usize) -> Vec<&BeaconItem> {
        let mut sorted: Vec<&BeaconItem> = self.items.iter().collect();
        sorted.sort_by(|a, b| b.priority.cmp(&a.priority));
        sorted.truncate(max);
        sorted
    }
}

/// The beacon component.
pub struct Beacon;

impl Beacon {
    /// Render the beacon into a frame.
    pub fn render(props: &BeaconProps<'_>, theme: &Theme) -> Frame {
        let state = &props.state;
        let max_items = theme.beacon.max_items;

        let mut frame = Frame::new();

        // Tree items first (grow upward from the brand line)
        let visible = state.visible_items(max_items);
        let tree_items: Vec<TreeItem> = visible
            .into_iter()
            .map(|item| TreeItem {
                status: item.status,
                message: item.message.clone(),
                metadata: item.metadata.clone(),
                detail: item.detail.clone(),
            })
            .collect();

        let tree_lines = Tree::render(&tree_items, theme);
        frame = frame.lines(tree_lines);

        // Brand line at the bottom — the pet anchors the beacon
        let header = render_header(state, props.pulse, theme);
        frame = frame.line(header);

        frame
    }
}

/// Props for the Beacon component.
pub struct BeaconProps<'a> {
    pub state: BeaconState,
    /// Pulse animation (None for static rendering).
    pub pulse: Option<&'a Pulse>,
}

fn render_header(state: &BeaconState, pulse: Option<&'_ Pulse>, theme: &Theme) -> String {
    let mut header = String::new();

    // Bar character: animated pulse when active, medium (home) when idle
    let bar = if state.is_active {
        if let Some(p) = pulse {
            p.render_bar(theme)
        } else {
            let color = bar_color(state.severity, theme);
            Pulse::render_home(theme, &color)
        }
    } else {
        let color = bar_color(state.severity, theme);
        Pulse::render_home(theme, &color)
    };

    header.push_str(&bar);
    header.push(' ');

    // Rainbow brand text
    header.push_str(&Rainbow::render(&state.brand, theme));

    // Phase label
    if let Some(ref phase) = state.phase {
        header.push_str("  ");
        let phase_icon = if state.is_active {
            format!(
                "{}{}{}",
                theme.palette.warning.fg_code(),
                theme.icons.for_status(StatusIcon::InProgress),
                Theme::reset()
            )
        } else if state.severity == Severity::Error {
            format!(
                "{}{}{}",
                theme.palette.error.fg_code(),
                theme.icons.for_status(StatusIcon::Failed),
                Theme::reset()
            )
        } else {
            format!(
                "{}{}{}",
                theme.palette.success.fg_code(),
                theme.icons.for_status(StatusIcon::Success),
                Theme::reset()
            )
        };
        header.push_str(&phase_icon);
        header.push(' ');
        header.push_str(phase);
    }

    // Progress counter
    if let Some(ref progress) = state.progress {
        header.push_str(&format!("  {}{}{}", Theme::dim(), progress, Theme::reset()));
    }

    // Elapsed time
    if let Some(ref elapsed) = state.elapsed {
        header.push_str(&format!("  {}{}{}", Theme::dim(), elapsed, Theme::reset()));
    }

    header
}

fn bar_color(severity: Severity, theme: &Theme) -> crate::tokens::Color {
    match severity {
        Severity::Ok => theme.palette.bar_idle,
        Severity::Warning => theme.palette.bar_warning,
        Severity::Error => theme.palette.bar_error,
    }
}

/// Render the beacon in static mode (no animation, for shell hooks).
pub fn render_static(state: &BeaconState, theme: &Theme) -> Frame {
    Beacon::render(
        &BeaconProps {
            state: state.clone(),
            pulse: None,
        },
        theme,
    )
}

/// Render the beacon with a live pulse.
pub fn render_live(state: &BeaconState, pulse: &Pulse, theme: &Theme) -> Frame {
    Beacon::render(
        &BeaconProps {
            state: state.clone(),
            pulse: Some(pulse),
        },
        theme,
    )
}

/// Render the beacon in CI mode (prefixed lines, no tree chars).
/// Items first, then header — same bottom-anchored order.
pub fn render_ci(state: &BeaconState, prefix: &str) -> Vec<String> {
    let mut lines = Vec::new();

    // Items first (plain text, no tree chars)
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

    // Header at the bottom
    if let Some(ref phase) = state.phase {
        let progress = state.progress.as_deref().unwrap_or("");
        let elapsed = state.elapsed.as_deref().unwrap_or("");
        lines.push(format!("[{prefix}] {phase} {progress} {elapsed}").trim().to_string());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_static_beacon() {
        let theme = Theme::default();
        let state = BeaconState {
            brand: "test".into(),
            phase: Some("Done".into()),
            items: vec![BeaconItem {
                status: StatusIcon::Success,
                message: "task completed".into(),
                metadata: Some("1.2s".into()),
                detail: None,
                priority: 10,
            }],
            severity: Severity::Ok,
            ..BeaconState::default()
        };
        let frame = render_static(&state, &theme);
        assert!(!frame.is_empty());
        assert!(frame.lines.len() >= 2); // header + 1 item
    }

    #[test]
    fn visible_items_respects_max() {
        let state = BeaconState {
            items: (0..10)
                .map(|i| BeaconItem {
                    status: StatusIcon::Success,
                    message: format!("item {i}"),
                    metadata: None,
                    detail: None,
                    priority: i as u8,
                })
                .collect(),
            ..BeaconState::default()
        };
        let visible = state.visible_items(5);
        assert_eq!(visible.len(), 5);
        // Highest priority first
        assert_eq!(visible[0].priority, 9);
    }

    #[test]
    fn ci_mode_renders_plain() {
        let state = BeaconState {
            phase: Some("Building...".into()),
            progress: Some("2/5 tasks".into()),
            items: vec![BeaconItem {
                status: StatusIcon::Success,
                message: "task done".into(),
                metadata: None,
                detail: None,
                priority: 10,
            }],
            ..BeaconState::default()
        };
        let lines = render_ci(&state, "cimera");
        // Items first, header last (bottom-anchored)
        assert!(lines[0].contains("task done"));
        assert!(lines.last().expect("should have lines").contains("Building"));
        assert!(!lines[0].contains('\x1b'));
    }
}
