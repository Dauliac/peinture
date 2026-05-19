//! Output context detection — TTY, NO_COLOR, CI, terminal dimensions.

use console::Term;

/// Runtime output context, detected once at startup.
#[derive(Debug, Clone)]
pub struct OutputContext {
    /// Whether stderr is a TTY (interactive terminal).
    pub is_tty: bool,
    /// Whether NO_COLOR env var is set (https://no-color.org/).
    pub no_color: bool,
    /// Whether CLICOLOR_FORCE=1 is set (force colors even in pipe).
    pub clicolor_force: bool,
    /// Whether running in CI (CI env var is set).
    pub is_ci: bool,
    /// Whether terminal is dumb (TERM=dumb).
    pub is_dumb: bool,
    /// Terminal width in columns.
    pub term_width: u16,
    /// Terminal height in rows.
    pub term_height: u16,
}

impl OutputContext {
    /// Detect the current output context from the environment.
    pub fn detect() -> Self {
        let term = Term::stderr();
        let (height, width) = term.size();
        Self {
            is_tty: term.is_term(),
            no_color: std::env::var("NO_COLOR").is_ok(),
            clicolor_force: std::env::var("CLICOLOR_FORCE")
                .map(|v| v == "1")
                .unwrap_or(false),
            is_ci: std::env::var("CI").is_ok(),
            is_dumb: std::env::var("TERM")
                .map(|v| v == "dumb")
                .unwrap_or(false),
            term_width: width,
            term_height: height,
        }
    }

    /// Whether colors should be used.
    pub fn use_colors(&self) -> bool {
        if self.clicolor_force {
            return true;
        }
        if self.no_color || self.is_dumb {
            return false;
        }
        self.is_tty
    }

    /// Whether animations (pulse, spinners) should run.
    pub fn use_animations(&self) -> bool {
        self.is_tty && !self.is_ci && !self.is_dumb && !self.no_color
    }

    /// Whether the pinned beacon region should be used.
    pub fn use_pinned_region(&self) -> bool {
        self.is_tty && !self.is_ci
    }

    /// Refresh terminal dimensions (call on SIGWINCH or periodically).
    pub fn refresh_size(&mut self) {
        let term = Term::stderr();
        let (height, width) = term.size();
        self.term_width = width;
        self.term_height = height;
    }
}

impl Default for OutputContext {
    fn default() -> Self {
        Self::detect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_does_not_panic() {
        let _ctx = OutputContext::detect();
    }

    #[test]
    fn no_color_disables_colors() {
        let ctx = OutputContext {
            is_tty: true,
            no_color: true,
            clicolor_force: false,
            is_ci: false,
            is_dumb: false,
            term_width: 80,
            term_height: 24,
        };
        assert!(!ctx.use_colors());
    }

    #[test]
    fn clicolor_force_overrides_no_color() {
        let ctx = OutputContext {
            is_tty: false,
            no_color: true,
            clicolor_force: true,
            is_ci: false,
            is_dumb: false,
            term_width: 80,
            term_height: 24,
        };
        assert!(ctx.use_colors());
    }

    #[test]
    fn ci_disables_animation() {
        let ctx = OutputContext {
            is_tty: true,
            no_color: false,
            clicolor_force: false,
            is_ci: true,
            is_dumb: false,
            term_width: 80,
            term_height: 24,
        };
        assert!(!ctx.use_animations());
        assert!(!ctx.use_pinned_region());
    }
}
