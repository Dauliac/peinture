//! Demo: rainbow text rendering with different themes.
#![allow(clippy::print_stdout)]

use peinture::tokens::{Palette, Theme};
use peinture::component::Rainbow;

fn main() {
    println!("=== Rainbow Demo ===\n");

    // Default theme (pastel rainbow)
    let theme = Theme::default();
    println!("Default rainbow: {}", Rainbow::render("cimera", &theme));
    println!("Long text:       {}", Rainbow::render("hello world from peinture!", &theme));

    // Custom rainbow palette
    let mut custom_palette = Palette::default();
    custom_palette.rainbow = vec![
        peinture::tokens::Color::Named(peinture::tokens::palette::NamedColor::Red),
        peinture::tokens::Color::Named(peinture::tokens::palette::NamedColor::Green),
        peinture::tokens::Color::Named(peinture::tokens::palette::NamedColor::Blue),
    ];
    println!(
        "RGB rainbow:     {}",
        Rainbow::render_with_palette("peinture", &custom_palette)
    );

    // Plain mode (no colors)
    let plain = Palette::plain();
    println!(
        "Plain mode:      {}",
        Rainbow::render_with_palette("cimera", &plain)
    );

    println!("\nDone.");
}
