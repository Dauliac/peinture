//! Synchronized update protocol (iTerm2 / DEC private mode 2026).
//!
//! Wraps terminal writes in begin/end markers so the terminal
//! holds display until the full frame is ready. Prevents flicker
//! during cursor-up + clear + redraw cycles.
//!
//! Terminals that don't support this simply ignore the escape sequences.

/// Begin synchronized update — terminal holds display.
pub const SYNC_BEGIN: &str = "\x1b[?2026h";

/// End synchronized update — terminal flushes buffered content.
pub const SYNC_END: &str = "\x1b[?2026l";

/// Wrap content in synchronized update markers.
pub fn wrap_sync(content: &str) -> String {
    format!("{SYNC_BEGIN}{content}{SYNC_END}")
}
