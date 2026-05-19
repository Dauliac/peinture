//! Terminal detection and low-level output management.

pub mod context;
pub mod painter;
#[cfg(feature = "pty")]
pub mod pty_capture;
pub mod sync_update;

pub use context::OutputContext;
pub use painter::Painter;
#[cfg(feature = "pty")]
pub use pty_capture::PtyCapture;
