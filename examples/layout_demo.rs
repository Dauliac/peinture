//! Demo: layout containers (VStack, HStack).
#![allow(clippy::print_stdout)]

use peinture::component::Text;
use peinture::layout::{HStack, VStack};
use peinture::tokens::{Semantic, Theme};
use peinture::traits::Render;

fn main() {
    let theme = Theme::default();

    println!("=== Layout Demo ===\n");

    // VStack
    println!("VStack:");
    let v = VStack::new()
        .child(Text::new("Header").bold())
        .child(Text::dim("  body line 1"))
        .child(Text::dim("  body line 2"))
        .child(Text::new("Footer").color(Semantic::Success));

    for line in &v.render(&theme).lines {
        println!("{line}");
    }

    println!();

    // HStack
    println!("HStack (gap=4):");
    let h = HStack::new()
        .gap(4)
        .child(Text::new("LEFT"))
        .child(Text::new("RIGHT").color(Semantic::Info));

    for line in &h.render(&theme).lines {
        println!("{line}");
    }

    println!("\nDone.");
}
