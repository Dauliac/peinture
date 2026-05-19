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
    pub branch: String,
    pub last: String,
    pub vertical: String,
    pub blank: String,
}

impl Default for TreeChars {
    fn default() -> Self {
        Self {
            branch: "\u{251C}\u{2500}".into(),   // |-
            last: "\u{2514}\u{2500}".into(),      // '-
            vertical: "\u{2502}".into(),           // |
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
            vertical: "|".into(),
            blank: " ".into(),
        }
    }

    /// Connector for a tree item.
    pub fn connector(&self, is_last: bool) -> &str {
        if is_last { &self.last } else { &self.branch }
    }

    /// Continuation line prefix.
    pub fn continuation(&self, is_last: bool) -> &str {
        if is_last { &self.blank } else { &self.vertical }
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
