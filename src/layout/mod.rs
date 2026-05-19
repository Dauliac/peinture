//! Layout primitives — composing components spatially.
//!
//! Inspired by CSS flexbox and SwiftUI stacks:
//! - **VStack** — vertical stack (components stacked top-to-bottom)
//! - **HStack** — horizontal stack (components side-by-side)
//! - **Fixed** — component pinned to a position (bottom, top)
//! - **Flex** — component that fills available space

use crate::component::Frame;

/// Vertical stack — concatenate frames top-to-bottom.
pub fn vstack(frames: &[Frame]) -> Frame {
    let mut result = Frame::new();
    for frame in frames {
        result = result.lines(frame.lines.iter().cloned());
    }
    result
}

/// Horizontal stack — place frames side-by-side.
/// Pads shorter frames with blank lines to match the tallest.
pub fn hstack(frames: &[Frame], gap: usize) -> Frame {
    if frames.is_empty() {
        return Frame::new();
    }

    let max_height = frames.iter().map(|f| f.height()).max().unwrap_or(0);
    let gap_str = " ".repeat(gap);

    // Calculate max visible width for each frame
    let widths: Vec<usize> = frames
        .iter()
        .map(|f| {
            f.lines
                .iter()
                .map(|l| console::measure_text_width(l))
                .max()
                .unwrap_or(0)
        })
        .collect();

    let mut result = Frame::new();
    for row in 0..max_height {
        let mut line = String::new();
        for (col, frame) in frames.iter().enumerate() {
            if col > 0 {
                line.push_str(&gap_str);
            }
            if row < frame.height() {
                line.push_str(&frame.lines[row]);
                // Pad to column width for alignment
                let visible = console::measure_text_width(&frame.lines[row]);
                if col < frames.len() - 1 && visible < widths[col] {
                    line.push_str(&" ".repeat(widths[col] - visible));
                }
            } else if col < frames.len() - 1 {
                line.push_str(&" ".repeat(widths[col]));
            }
        }
        result = result.line(line);
    }
    result
}

/// Pad a frame with blank lines to reach a target height.
pub fn pad_height(frame: &Frame, target_height: usize) -> Frame {
    let mut result = frame.clone();
    while result.lines.len() < target_height {
        result.lines.push(String::new());
    }
    result
}

/// Truncate a frame to a maximum height.
pub fn truncate_height(frame: &Frame, max_height: usize) -> Frame {
    let mut result = frame.clone();
    result.lines.truncate(max_height);
    result
}

/// Add left padding to each line of a frame.
pub fn pad_left(frame: &Frame, padding: usize) -> Frame {
    let pad = " ".repeat(padding);
    Frame {
        lines: frame.lines.iter().map(|l| format!("{pad}{l}")).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vstack_concatenates() {
        let a = Frame::new().line("line 1".into());
        let b = Frame::new().line("line 2".into());
        let result = vstack(&[a, b]);
        assert_eq!(result.height(), 2);
    }

    #[test]
    fn hstack_aligns() {
        let a = Frame::new().line("left".into());
        let b = Frame::new().line("right".into());
        let result = hstack(&[a, b], 2);
        assert_eq!(result.height(), 1);
        assert!(result.lines[0].contains("left"));
        assert!(result.lines[0].contains("right"));
    }

    #[test]
    fn truncate_limits_height() {
        let frame = Frame::new()
            .line("1".into())
            .line("2".into())
            .line("3".into());
        let result = truncate_height(&frame, 2);
        assert_eq!(result.height(), 2);
    }
}
