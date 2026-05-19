//! Demo: streaming output with pinned beacon at bottom.
//!
//! Shows how the painter manages the two zones:
//! build output scrolling up, beacon fixed at bottom.
#![allow(clippy::print_stderr, clippy::print_stdout)]

use peinture::component::beacon::{self, BeaconState, Severity};
use peinture::component::BeaconItem;
use peinture::terminal::OutputContext;
use peinture::terminal::painter::Painter;
use peinture::tokens::Theme;
use peinture::tokens::icons::StatusIcon;
use std::thread;
use std::time::Duration;

fn main() {
    let ctx = OutputContext::detect();
    if !ctx.use_pinned_region() {
        println!("This demo requires an interactive terminal (TTY).");
        println!("Run directly, not piped.");
        return;
    }

    let theme = Theme::default();
    let mut painter = Painter::new(ctx.term_width, ctx.term_height);
    painter.hide_cursor();

    let deps = [
        "serde", "tokio", "hyper", "tonic", "prost", "tracing",
        "clap", "thiserror", "anyhow", "uuid", "chrono", "regex",
        "rand", "base64", "flate2", "rusqlite", "console", "glob",
    ];

    // Single pulse for the whole animation
    let pulse = peinture::component::pulse::Pulse::new();

    let mut state = BeaconState {
        brand: "cimera".into(),
        phase: Some("Building...".into()),
        progress: Some("0/1 tasks".into()),
        is_active: true,
        severity: Severity::Ok,
        items: vec![BeaconItem {
            status: StatusIcon::InProgress,
            message: "nix build: myservice:rust".into(),
            metadata: None,
            detail: None,
            priority: 10,
        }],
        ..BeaconState::default()
    };

    for (i, dep) in deps.iter().enumerate() {
        // Stream a compilation line
        painter.stream_line(format!(
            "   \x1b[32mCompiling\x1b[0m {dep} v{}.{}.{}",
            i / 6,
            i % 6,
            i * 7 % 100
        ));

        // Update beacon elapsed
        state.elapsed = Some(format!("{:.1}s", i as f32 * 0.8));

        let frame = beacon::render_live(&state, &pulse, &theme);
        painter.render_frame(&frame.lines);

        thread::sleep(Duration::from_millis(300));
    }

    // Completion
    state.phase = Some("Done".into());
    state.is_active = false;
    state.items[0].status = StatusIcon::Success;
    state.items[0].message = "myservice:rust".into();
    state.items[0].metadata = Some("14.4s".into());

    let frame = beacon::render_static(&state, &theme);
    painter.print_final(&frame.lines);
    println!();
}
