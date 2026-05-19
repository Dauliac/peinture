//! Component system — Vue/React-inspired composable UI elements.
//!
//! # Architecture
//!
//! Components are pure functions: `(State, Theme) -> Frame`.
//! A `Frame` is a list of styled lines ready for the painter.
//!
//! Components compose via the layout system (VStack, HStack, etc.)
//! but can also be rendered standalone.
//!
//! # Built-in Components
//!
//! - **Beacon** — pinned status panel with tree, pulse, header
//! - **Rainbow** — multi-color text from palette
//! - **Pulse** — animated heartbeat bar
//! - **Tree** — hierarchical item list with connectors

pub mod beacon;
pub mod pulse;
pub mod rainbow;
pub mod tree;

pub use beacon::{Beacon, BeaconItem, BeaconState, Severity};
pub use pulse::Pulse;
pub use rainbow::Rainbow;
pub use tree::Tree;

use crate::tokens::Theme;

/// A rendered frame — list of lines ready for the painter.
#[derive(Debug, Clone, Default)]
pub struct Frame {
    pub lines: Vec<String>,
}

impl Frame {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    pub fn line(mut self, line: String) -> Self {
        self.lines.push(line);
        self
    }

    pub fn lines(mut self, lines: impl IntoIterator<Item = String>) -> Self {
        self.lines.extend(lines);
        self
    }

    pub fn height(&self) -> usize {
        self.lines.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// Trait for renderable components.
///
/// Components are stateless renderers: they take their state as input
/// and produce a `Frame` as output. State management is external
/// (the application owns the state and passes it to render calls).
pub trait Component {
    /// The state/props this component needs to render.
    type Props;

    /// Render the component into a frame of styled lines.
    fn render(props: &Self::Props, theme: &Theme) -> Frame;
}
