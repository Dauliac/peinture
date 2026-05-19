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
    /// Bar characters for pulse animation: [thick, medium, thin].
    pub bar_chars: [char; 3],
}

impl Default for BeaconTokens {
    fn default() -> Self {
        Self {
            max_items: 5,
            fps: 5,
            pulse_cycle_ms: 2000,
            bar_chars: [
                '\u{258A}', // left 3/4 block (chunky, resting)
                '\u{258C}', // left 1/2 block (medium)
                '\u{258E}', // left 1/4 block (thin, peak)
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
    fn default_bar_chars() {
        let t = BeaconTokens::default();
        assert_eq!(t.bar_chars[0], '\u{258A}');
        assert_eq!(t.bar_chars[2], '\u{258E}');
    }
}
