//! Demo: streaming output with pinned beacon at bottom.
#![allow(clippy::print_stderr, clippy::print_stdout)]

use peinture::component::beacon::{self, BeaconState, Severity};
use peinture::component::BeaconItem;
use peinture::terminal::OutputContext;
use peinture::terminal::painter::Painter;
use peinture::tokens::Theme;
use peinture::tokens::icons::StatusIcon;
use std::time::{Duration, Instant};
use std::thread;

fn main() {
    let ctx = OutputContext::detect();
    if !ctx.use_pinned_region() {
        println!("This demo requires an interactive terminal (TTY).");
        return;
    }

    let theme = Theme::default();
    let reserved = (theme.beacon.max_items + 1) as u16;
    let mut painter = Painter::new(ctx.term_width, ctx.term_height, reserved);
    painter.hide_cursor();
    let start = Instant::now();

    let deps = [
        "serde", "tokio", "hyper", "tonic", "prost", "tracing",
        "clap", "thiserror", "anyhow", "uuid", "chrono", "regex",
        "rand", "base64", "flate2", "rusqlite", "console", "glob",
    ];

    let mut state = BeaconState {
        brand: "cimera".into(),
        phase: Some("Building...".into()),
        progress: Some("0/1 tasks".into()),
        is_active: true,
        severity: Severity::Ok,
        items: vec![
            BeaconItem::workload(StatusIcon::InProgress, "nix build: myservice:rust"),
        ],
        ..BeaconState::default()
    };

    for (i, dep) in deps.iter().enumerate() {
        painter.stream_line(format!(
            "   \x1b[32mCompiling\x1b[0m {dep} v{}.{}.{}",
            i / 6, i % 6, i * 7 % 100
        ));
        state.elapsed = Some(format!("{:.1}s", i as f32 * 0.8));

        let frame = beacon::render_live(&state, start, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(300));
    }

    // Finish pulse before releasing
    state.phase = Some("Done".into());
    state.items[0].status = StatusIcon::Success;
    state.items[0].message = "myservice:rust".into();
    state.items[0].metadata = Some("14.4s".into());

    loop {
        let frame = beacon::render_live(&state, start, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(theme.beacon.frame_interval_ms()));
        if beacon::is_at_rest(start, &theme) { break; }
    }

    state.is_active = false;
    let frame = beacon::render_static(&state, &theme);
    painter.print_final(&frame.lines);
    println!();
}
