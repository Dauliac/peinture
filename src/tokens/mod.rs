//! Design tokens — single source of truth for all visual properties.
//!
//! Three-tier token hierarchy:
//! 1. **Primitive** — raw colors, ANSI codes, character constants
//! 2. **Semantic** — meaning-based mappings (success, error, warning)
//! 3. **Component** — per-component configuration (beacon.max_items, pulse.fps)

pub mod beacon_tokens;
pub mod icons;
pub mod palette;
pub mod semantic;
pub mod spacing;
pub mod theme;

pub use beacon_tokens::BeaconTokens;
pub use icons::{IconSet, StatusIcon, TreeChars};
pub use palette::{Color, Palette};
pub use semantic::Semantic;
pub use spacing::Spacing;
pub use theme::Theme;
