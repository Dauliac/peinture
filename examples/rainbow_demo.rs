//! Demo: text component with rainbow, semantic colors, bold, dim.
#![allow(clippy::print_stdout)]

use peinture::component::Text;
use peinture::tokens::{Semantic, Theme};
use peinture::traits::Render;

fn main() {
    let theme = Theme::default();

    println!("=== Text Component Demo ===\n");

    let rainbow = Text::rainbow("cimera");
    println!("Rainbow:   {}", rainbow.render(&theme).lines[0]);

    let success = Text::new("build succeeded").color(Semantic::Success);
    println!("Success:   {}", success.render(&theme).lines[0]);

    let error = Text::new("build failed").color(Semantic::Error).bold();
    println!("Error:     {}", error.render(&theme).lines[0]);

    let warning = Text::new("deprecated API").color(Semantic::Warning);
    println!("Warning:   {}", warning.render(&theme).lines[0]);

    let dim = Text::dim("(2.1s)");
    println!("Dim:       {}", dim.render(&theme).lines[0]);

    let plain_theme = Theme::plain();
    let plain = Text::rainbow("cimera");
    println!("\nPlain:     {}", plain.render(&plain_theme).lines[0]);

    println!("\nDone.");
}
