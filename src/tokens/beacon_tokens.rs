//! Beacon-specific component tokens.

use serde::Deserialize;

/// Configuration tokens for the Beacon component.
#[derive(Debug, Clone, Deserialize)]
pub struct BeaconTokens {
    /// Maximum number of tree items displayed (default: 5).
    pub max_items: usize,
    /// Render frames per second (default: 5).
    pub fps: u8,
    /// Pulse breathing cycle duration in milliseconds (default: 2000).
    pub pulse_cycle_ms: u32,
    /// Bar strings for pulse animation: [large, medium, small].
    /// Each is a multi-char string for centered expansion.
    /// Default: ["██", "▐▌", "▕▏"]
    pub bar_frames: [String; 3],
}

impl Default for BeaconTokens {
    fn default() -> Self {
        Self {
            max_items: 5,
            fps: 5,
            pulse_cycle_ms: 2000,
            bar_frames: [
                "\u{2588}\u{2588}".into(),  // ██  full (large, expand)
                "\u{2590}\u{258C}".into(),  // ▐▌  half (medium, home)
                "\u{2595}\u{258F}".into(),  // ▕▏  thin (small, contract)
            ],
        }
    }
}

impl BeaconTokens {
    /// Frame interval in milliseconds, derived from fps.
    pub fn frame_interval_ms(&self) -> u64 {
        if self.fps == 0 { return 200; }
        1000 / self.fps as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_interval() {
        let t = BeaconTokens::default();
        assert_eq!(t.frame_interval_ms(), 200); // 1000/5 = 200ms
    }

    #[test]
    fn default_bar_frames() {
        let t = BeaconTokens::default();
        assert_eq!(t.bar_frames[0], "██");
        assert_eq!(t.bar_frames[1], "▐▌");
        assert_eq!(t.bar_frames[2], "▕▏");
    }
}
