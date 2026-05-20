//! Beacon-specific component tokens.

use serde::Deserialize;

/// Configuration tokens for the Beacon component.
#[derive(Debug, Clone, Deserialize)]
pub struct BeaconTokens {
    /// Maximum number of tree items displayed (default: 5).
    pub max_items: usize,
    /// Render frames per second (default: 12).
    pub fps: u8,
    /// Pulse breathing cycle duration in milliseconds (default: 2200).
    pub pulse_cycle_ms: u32,
    /// Notification time-to-live in milliseconds (default: 10000 = 10s).
    /// After this, the notification is removed.
    pub notification_ttl_ms: u64,
    /// Fraction of TTL at which fade starts (default: 0.6 = fade begins at 60% of TTL).
    pub notification_fade_start: f32,
    /// Bar frames for pulse animation — ordered from smallest to largest.
    /// Each is a 2-char string for centered expansion.
    pub bar_frames: Vec<String>,
    /// Index into `bar_frames` for the resting/home position.
    pub bar_home_idx: usize,
}

impl Default for BeaconTokens {
    fn default() -> Self {
        Self {
            max_items: 5,
            fps: 12,
            pulse_cycle_ms: 2200,
            notification_ttl_ms: 6_000,
            notification_fade_start: 0.85,
            // 5 stages — left column is always █ (matches ├ width)
            // Pulse expands RIGHT into the second column
            // Left edge perfectly aligned with tree connectors
            bar_frames: vec![
                "\u{2588} ".into(), // █   0  HOME (full block + space)
                "\u{2588}\u{258F}".into(), // █▏  1
                "\u{2588}\u{258E}".into(), // █▎  2
                "\u{2588}\u{258D}".into(), // █▍  3  PEAK
            ],
            bar_home_idx: 0,
        }
    }
}

impl BeaconTokens {
    /// Frame interval in milliseconds, derived from fps.
    pub fn frame_interval_ms(&self) -> u64 {
        if self.fps == 0 { return 200; }
        1000 / self.fps as u64
    }

    /// The home/resting bar frame.
    pub fn home_frame(&self) -> &str {
        &self.bar_frames[self.bar_home_idx]
    }

    /// Map a displacement value (-1.0..+1.0) to a bar frame.
    /// 0.0 = home, +1.0 = largest, -1.0 = smallest.
    pub fn frame_for_displacement(&self, displacement: f32) -> &str {
        let home = self.bar_home_idx as f32;
        let max_idx = (self.bar_frames.len() - 1) as f32;

        let idx = if displacement >= 0.0 {
            home + displacement * (max_idx - home)
        } else {
            home + displacement * home
        };

        let idx = idx.round().clamp(0.0, max_idx) as usize;
        &self.bar_frames[idx]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_interval() {
        let t = BeaconTokens::default();
        assert_eq!(t.frame_interval_ms(), 83); // 1000/12
    }

    #[test]
    fn home_frame_is_full_block_space() {
        let t = BeaconTokens::default();
        assert_eq!(t.home_frame(), "\u{2588} "); // █
    }

    #[test]
    fn displacement_zero_is_home() {
        let t = BeaconTokens::default();
        assert_eq!(t.frame_for_displacement(0.0), t.home_frame());
    }

    #[test]
    fn displacement_max_is_peak() {
        let t = BeaconTokens::default();
        assert_eq!(t.frame_for_displacement(1.0), "\u{2588}\u{258D}"); // █▍
    }

    #[test]
    fn displacement_min_is_home() {
        // No contraction below home — clamps to home
        let t = BeaconTokens::default();
        assert_eq!(t.frame_for_displacement(-1.0), t.home_frame());
    }
}
