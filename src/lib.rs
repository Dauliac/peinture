//! # peinture
//!
//! Terminal UI component library with design tokens, layered rendering,
//! and nom-style pinned regions.
//!
//! ## Architecture
//!
//! ```text
//! +--------------------------------------------------+
//! |                  Application                      |
//! |  Uses: Beacon, StreamWall, custom components      |
//! +-------------------+------------------------------+
//! |                   |                              |
//! |   Components      |      Layout                  |
//! |   Beacon          |      VStack / HStack         |
//! |   Rainbow         |      Fixed / Flex            |
//! |   Tree            |      Padding / Align         |
//! |   Pulse           |                              |
//! +-------------------+------------------------------+
//! |                                                  |
//! |              Renderer (Painter)                   |
//! |   nom-style cursor-up/clear, sync updates,       |
//! |   layers (stream + pinned), resize handling       |
//! +--------------------------------------------------+
//! |                                                  |
//! |            Design Tokens (Theme)                  |
//! |   Palette, Spacing, Icons, Component tokens       |
//! |   Loadable from TOML files                        |
//! +--------------------------------------------------+
//! |                                                  |
//! |          Terminal (console crate)                  |
//! |   OutputContext, TTY detect, NO_COLOR, resize      |
//! +--------------------------------------------------+
//! ```
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use peinture::{Theme, Painter, Beacon, BeaconState};
//!
//! let theme = Theme::default();
//! let mut painter = Painter::new(80, 24);
//! // ... see examples/ for full usage
//! ```

pub mod component;
pub mod layout;
pub mod renderer;
pub mod terminal;
pub mod tokens;

// Re-exports for convenience
pub use component::{Beacon, BeaconItem, BeaconState, Component, Frame};
pub use renderer::Painter;
pub use terminal::OutputContext;
pub use tokens::Theme;
