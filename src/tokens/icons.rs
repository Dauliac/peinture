//! Icon and symbol tokens.

use serde::Deserialize;

/// Status icon for a line item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusIcon {
    Success,
    Failed,
    InProgress,
    Pending,
    Cached,
    Skipped,
    Warning,
    Info,
}

/// Icon set — maps status to Unicode characters.
#[derive(Debug, Clone, Deserialize)]
pub struct IconSet {
    pub success: String,
    pub failed: String,
    pub in_progress: String,
    pub pending: String,
    pub cached: String,
    pub skipped: String,
    pub warning: String,
    pub info: String,
}

impl Default for IconSet {
    fn default() -> Self {
        Self {
            success: "\u{2713}".into(),     // checkmark
            failed: "\u{2717}".into(),      // ballot x
            in_progress: "\u{25D0}".into(), // circle left half
            pending: "\u{25CB}".into(),     // white circle
            cached: "\u{25CF}".into(),      // black circle
            skipped: "\u{2298}".into(),     // circled division slash
            warning: "\u{26A0}".into(),     // warning sign
            info: "\u{2139}".into(),        // info source
        }
    }
}

impl IconSet {
    /// Get the icon string for a status.
    pub fn for_status(&self, status: StatusIcon) -> &str {
        match status {
            StatusIcon::Success => &self.success,
            StatusIcon::Failed => &self.failed,
            StatusIcon::InProgress => &self.in_progress,
            StatusIcon::Pending => &self.pending,
            StatusIcon::Cached => &self.cached,
            StatusIcon::Skipped => &self.skipped,
            StatusIcon::Warning => &self.warning,
            StatusIcon::Info => &self.info,
        }
    }

    /// ASCII fallback for TERM=dumb.
    pub fn ascii() -> Self {
        Self {
            success: "+".into(),
            failed: "x".into(),
            in_progress: "*".into(),
            pending: "o".into(),
            cached: "=".into(),
            skipped: "-".into(),
            warning: "!".into(),
            info: "i".into(),
        }
    }
}

/// Tree-drawing characters.
#[derive(Debug, Clone, Deserialize)]
pub struct TreeChars {
    /// Branch connector (middle items): ├─
    pub branch: String,
    /// Last item (bottom of top-rooted tree): └─
    pub last: String,
    /// First item (top of bottom-rooted tree): ┌─
    pub first: String,
    /// Vertical continuation: │
    pub vertical: String,
    /// Blank continuation (under last/first item):
    pub blank: String,
}

impl Default for TreeChars {
    fn default() -> Self {
        Self {
            branch: "\u{251C}\u{2500}".into(),   // ├─
            last: "\u{2514}\u{2500}".into(),      // └─
            first: "\u{250C}\u{2500}".into(),     // ┌─
            vertical: "\u{2502}".into(),           // │
            blank: " ".into(),
        }
    }
}

impl TreeChars {
    /// ASCII fallback.
    pub fn ascii() -> Self {
        Self {
            branch: "|-".into(),
            last: "`-".into(),
            first: ",-".into(),
            vertical: "|".into(),
            blank: " ".into(),
        }
    }

    /// Connector for a top-rooted tree (last item at bottom).
    pub fn connector(&self, is_last: bool) -> &str {
        if is_last { &self.last } else { &self.branch }
    }

    /// Connector for a bottom-rooted tree (first item at top, root at bottom).
    /// First item gets ┌─, rest get ├─.
    pub fn connector_bottom_rooted(&self, is_first: bool) -> &str {
        if is_first { &self.first } else { &self.branch }
    }

    /// Continuation line prefix for a top-rooted tree.
    pub fn continuation(&self, is_last: bool) -> &str {
        if is_last { &self.blank } else { &self.vertical }
    }

    /// Continuation line prefix for a bottom-rooted tree.
    /// Always │ — all items connect down to the root (brand line).
    pub fn continuation_bottom_rooted(&self, _is_first: bool) -> &str {
        &self.vertical
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_for_status() {
        let icons = IconSet::default();
        assert_eq!(icons.for_status(StatusIcon::Success), "\u{2713}");
        assert_eq!(icons.for_status(StatusIcon::Failed), "\u{2717}");
    }

    #[test]
    fn tree_connector() {
        let tree = TreeChars::default();
        assert_eq!(tree.connector(false), "\u{251C}\u{2500}");
        assert_eq!(tree.connector(true), "\u{2514}\u{2500}");
    }
}
