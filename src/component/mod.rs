//! Component library — declarative, composable terminal UI elements.
//!
//! All components implement the `Render` trait and compose via
//! the builder pattern. No proc macros needed.
//!
//! # Built-in Components
//!
//! | Component | Purpose | Key trait |
//! |-----------|---------|-----------|
//! | `Text`    | Styled text span | `Render` |
//! | `Bar`     | Animated pulse bar | `Render + Animate` |
//! | `Tree`    | Hierarchical items with connectors | `Render` |
//! | `Beacon`  | Composite: tree + header (pinned) | `Render` |

pub mod bar;
pub mod beacon;
pub mod frame;
pub mod text;
pub mod tree;

pub use bar::Bar;
pub use beacon::{Beacon, BeaconItem, BeaconState, ItemKind, Orientation, Severity};
pub use frame::Frame;
pub use text::Text;
pub use tree::{Tree, TreeItem};
