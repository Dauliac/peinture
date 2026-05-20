//! Demo: capture a screen-rewriting program via PTY and overlay a peinture
//! beacon below it.
//!
//! Usage:
//!   cargo run --features pty --example pty_demo -- <command> [args...]
//!
//! Examples:
//!   # Overlay beacon on top of nix-output-monitor building the devShell:
//!   cargo run --features pty --example pty_demo -- \
//!       nom -- nix build '.#devShells.x86_64-linux.default'
//!
//!   # Any screen-rewriting program works:
//!   cargo run --features pty --example pty_demo -- htop
//!   cargo run --features pty --example pty_demo -- watch -n1 date
#![allow(clippy::print_stderr, clippy::print_stdout, clippy::unwrap_used)]

use peinture::component::beacon::{self, BeaconState, Severity};
use peinture::component::BeaconItem;
use peinture::terminal::OutputContext;
use peinture::terminal::painter::Painter;
use peinture::terminal::pty_capture::PtyCapture;
use peinture::tokens::Theme;
use peinture::tokens::icons::StatusIcon;
use std::time::{Duration, Instant};
use std::thread;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("Usage: pty_demo <command> [args...]");
        eprintln!();
        eprintln!("Example with nix-output-monitor:");
        eprintln!("  cargo run --features pty --example pty_demo -- \\");
        eprintln!("      nom -- nix build '.#devShells.x86_64-linux.default'");
        std::process::exit(1);
    }

    let ctx = OutputContext::detect();
    if !ctx.use_pinned_region() {
        eprintln!("This demo requires an interactive terminal (TTY).");
        std::process::exit(1);
    }

    let theme = Theme::default();
    let cmd_display = args.join(" ");

    // Reserve rows for the beacon (bar + header + up to 3 tree items).
    let beacon_reserve: u16 = 6;
    let capture_rows = ctx.term_height.saturating_sub(beacon_reserve).max(4);

    // Spawn the command in a virtual PTY.
    let str_args: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
    let mut capture = PtyCapture::spawn(&args[0], &str_args, capture_rows, ctx.term_width)
        .unwrap_or_else(|e| {
            eprintln!("Failed to spawn '{}': {e}", args[0]);
            std::process::exit(1);
        });

    let mut painter = Painter::new(ctx.term_width, ctx.term_height, beacon_reserve as usize);
    painter.hide_cursor();

    let start = Instant::now();

    let mut state = BeaconState {
        brand: "peinture".into(),
        phase: Some("Running...".into()),
        is_active: true,
        severity: Severity::Ok,
        items: vec![BeaconItem::workload(StatusIcon::InProgress, cmd_display.clone())],
        ..BeaconState::default()
    };

    // Main render loop.
    loop {
        let running = capture.process_available();

        // Update elapsed time.
        let elapsed = start.elapsed();
        state.elapsed = Some(format!("{:.1}s", elapsed.as_secs_f64()));

        // Get the captured program's screen (top portion).
        let mut screen = capture.screen_lines();

        // Pad or trim to exactly capture_rows lines so the beacon stays fixed.
        screen.resize(capture_rows as usize, String::new());

        // Render the peinture beacon (bottom portion).
        let beacon_frame = beacon::render_live(&state, start, &theme);

        // Combine: captured screen + beacon = full terminal frame.
        screen.extend(beacon_frame.lines.iter().cloned());

        painter.render_frame(&screen);

        if !running {
            break;
        }

        thread::sleep(Duration::from_millis(theme.beacon.frame_interval_ms()));
    }

    // Let the pulse finish its cycle.
    state.phase = Some("Done".into());
    state.items[0].status = StatusIcon::Success;
    state.items[0].metadata = Some(format!("{:.1}s", start.elapsed().as_secs_f64()));

    loop {
        state.elapsed = Some(format!("{:.1}s", start.elapsed().as_secs_f64()));
        let frame = beacon::render_live(&state, start, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(theme.beacon.frame_interval_ms()));
        if beacon::is_at_rest(start, &theme) {
            break;
        }
    }

    // Print final static beacon as scrollback.
    state.is_active = false;
    let frame = beacon::render_static(&state, &theme);
    painter.print_final(&frame.lines);
    println!();
}
