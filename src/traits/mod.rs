//! Core traits — composable behaviors for components.
//!
//! These traits are the backbone of peinture's component system.
//! Implement them to create custom components with consistent behavior.

use crate::component::Frame;
use crate::tokens::Theme;

/// Renderable component — can produce a Frame given a Theme.
///
/// This is the fundamental trait. Every visible component implements it.
/// Rendering is a pure function: same input → same output (deterministic).
///
/// ```rust,ignore
/// impl Render for MyComponent {
///     fn render(&self, theme: &Theme) -> Frame {
///         Frame::line(format!("Hello, {}!", self.name))
///     }
/// }
/// ```
pub trait Render {
    /// Render this component into a frame of styled lines.
    fn render(&self, theme: &Theme) -> Frame;

    /// Height in lines (for layout calculations).
    /// Default: renders and counts lines. Override for efficiency.
    fn height(&self, theme: &Theme) -> usize {
        self.render(theme).height()
    }
}

/// Animatable component — has time-dependent state.
///
/// Components with `Animate` produce different frames over time
/// (e.g., pulse bar, spinner, progress counter).
///
/// The animation is driven externally (by the render loop).
/// The component just reports its current visual state.
pub trait Animate {
    /// Whether the animation is currently active.
    fn is_active(&self) -> bool;

    /// Whether the animation has reached a rest/home position.
    /// Used to know when it's safe to stop cleanly.
    fn is_at_rest(&self) -> bool;
}

/// Composable — can contain children.
///
/// Containers (VStack, HStack) implement this.
/// Children are rendered and composed according to layout rules.
pub trait Container: Render {
    /// Add a child component.
    fn push(&mut self, child: Box<dyn Render>);
}

/// Styleable — can receive style overrides from parent context.
///
/// This enables the "cascade" pattern: parent themes flow down
/// to children unless explicitly overridden.
pub trait Styleable {
    /// Apply a style override. Returns self for chaining.
    fn with_theme(self, theme: &Theme) -> Self
    where
        Self: Sized;
}
