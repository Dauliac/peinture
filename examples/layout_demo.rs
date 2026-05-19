//! Demo: layout primitives (vstack, hstack, padding).
#![allow(clippy::print_stdout)]

use peinture::component::Frame;
use peinture::layout::{hstack, pad_left, vstack};

fn main() {
    println!("=== Layout Demo ===\n");

    // VStack
    let header = Frame::new().line("--- Header ---".into());
    let body = Frame::new()
        .line("  Line 1 of body".into())
        .line("  Line 2 of body".into());
    let footer = Frame::new().line("--- Footer ---".into());

    println!("VStack:");
    let vstacked = vstack(&[header, body, footer]);
    for line in &vstacked.lines {
        println!("{line}");
    }

    println!();

    // HStack
    let left = Frame::new()
        .line("LEFT-1".into())
        .line("LEFT-2".into())
        .line("LEFT-3".into());
    let right = Frame::new()
        .line("RIGHT-1".into())
        .line("RIGHT-2".into());

    println!("HStack (gap=4):");
    let hstacked = hstack(&[left, right], 4);
    for line in &hstacked.lines {
        println!("{line}");
    }

    println!();

    // Padding
    let content = Frame::new()
        .line("Indented content".into())
        .line("More indented".into());
    let padded = pad_left(&content, 6);

    println!("pad_left(6):");
    for line in &padded.lines {
        println!("{line}");
    }

    println!("\nDone.");
}
