//! Demo: terminal resize detection.
#![allow(clippy::print_stdout)]

use peinture::terminal::OutputContext;
use std::thread;
use std::time::Duration;

fn main() {
    println!("=== Resize Demo ===");
    println!("Resize your terminal window. Updates every second for 10s.\n");

    let mut ctx = OutputContext::detect();

    for i in 0..10 {
        ctx.refresh_size();
        println!(
            "[{:2}s] Terminal: {}x{} | TTY: {} | Colors: {} | Animations: {}",
            i, ctx.term_width, ctx.term_height,
            ctx.is_tty, ctx.use_colors(), ctx.use_animations(),
        );
        thread::sleep(Duration::from_secs(1));
    }
    println!("\nDone.");
}
