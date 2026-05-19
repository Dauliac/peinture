//! # peinture
//!
//! Declarative terminal UI component library.
//!
//! Think **Vue.js + Tailwind** for the terminal:
//! - Components are structs with trait-based behaviors
//! - Styling flows from design tokens (Theme) through `Styleable`
//! - Layout via composable containers (VStack, HStack, Fixed)
//! - Animations via the `Animate` trait
//! - Rendering is a pure function: `component.render(&theme) → Frame`
//!
//! ## Architecture
//!
//! ```text
//! ┌──────────────────────────────────────────────────┐
//! │ Application (your code)                          │
//! │   beacon.render(&theme) → Frame                  │
//! │   painter.draw(&frame)                           │
//! ├──────────────────────────────────────────────────┤
//! │ Components (composable, generic)                 │
//! │   Text, Bar, Tree, HStack, VStack                │
//! │   Built via builder pattern, not proc macros     │
//! ├──────────────────────────────────────────────────┤
//! │ Traits (behaviors)                               │
//! │   Render    → produces Frame                     │
//! │   Animate   → time-based state                   │
//! │   Styleable → reads Theme tokens                 │
//! │   Layout    → positions children                 │
//! ├──────────────────────────────────────────────────┤
//! │ Tokens (single source of truth)                  │
//! │   Theme { palette, spacing, icons, tree, ... }   │
//! │   Semantic colors: success, error, warning       │
//! │   Loadable from TOML                             │
//! ├──────────────────────────────────────────────────┤
//! │ Terminal (low-level output)                       │
//! │   Painter (nom-style pinned region)              │
//! │   OutputContext (TTY, NO_COLOR, resize)           │
//! └──────────────────────────────────────────────────┘
//! ```
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use peinture::prelude::*;
//!
//! let theme = Theme::default();
//! let text = Text::new("hello").color(Semantic::Success);
//! let frame = text.render(&theme);
//! ```

pub mod component;
pub mod layout;
pub mod renderer;
pub mod terminal;
pub mod tokens;
pub mod traits;

/// Prelude — import everything you need with `use peinture::prelude::*`.
pub mod prelude {
    pub use crate::component::*;
    pub use crate::layout::{HStack, VStack};
    pub use crate::terminal::{OutputContext, Painter};
    pub use crate::tokens::{Semantic, Theme};
    pub use crate::traits::{Animate, Render};
}

// Re-exports for convenience (non-prelude)
pub use component::{Beacon, BeaconItem, BeaconState};
pub use renderer::Painter;
pub use terminal::OutputContext;
pub use tokens::Theme;
