//! Demo: capture a screen-rewriting program via PTY and overlay a peinture
//! beacon below it. Scrolled-off content flows into real terminal scrollback.
//!
//! Layout:
//!   ─── top of terminal ───
//!   scrolled-off logs           ← stream zone (scrollback)
//!   ─── top of virtual window ──
//!   captured program screen     ← pinned region (live vt100)
//!   ─── bottom of virtual window
//!   beacon                      ← pinned region
//!   ─── bottom of terminal ───
//!
//! Usage:
//!   cargo run --features pty --example pty_demo -- <command> [args...]
//!
//! Examples:
//!   # Overlay beacon on top of nix-output-monitor:
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
use std::time::Instant;
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

    let mut ctx = OutputContext::detect();
    if !ctx.use_pinned_region() {
        eprintln!("This demo requires an interactive terminal (TTY).");
        std::process::exit(1);
    }

    let theme = Theme::default();
    let cmd_display = args.join(" ");

    let mut state = BeaconState {
        brand: "peinture".into(),
        phase: Some("Running...".into()),
        is_active: true,
        severity: Severity::Ok,
        items: vec![BeaconItem::workload(StatusIcon::InProgress, cmd_display.clone())],
        ..BeaconState::default()
    };

    // Compute beacon height to know how much space it needs.
    let beacon_height = beacon::render_live(&state, Instant::now(), &theme)
        .lines
        .len() as u16;

    // Virtual terminal: full terminal minus beacon minus 1 row for scroll zone.
    let capture_rows = ctx.term_height.saturating_sub(beacon_height + 1).max(4);

    let str_args: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
    let mut capture = PtyCapture::spawn(&args[0], &str_args, capture_rows, ctx.term_width)
        .unwrap_or_else(|e| {
            eprintln!("Failed to spawn '{}': {e}", args[0]);
            std::process::exit(1);
        });

    let pinned_height = capture_rows as usize + beacon_height as usize;
    let mut painter = Painter::new(ctx.term_width, ctx.term_height, pinned_height);
    painter.hide_cursor();

    let start = Instant::now();

    // ─── Main render loop ────────────────────────────────────────────────
    loop {
        let running = capture.process_available();

        // Handle terminal resize.
        ctx.refresh_size();
        let new_capture_rows = ctx.term_height.saturating_sub(beacon_height + 1).max(4);
        if new_capture_rows != capture.rows() || ctx.term_width != capture.cols() {
            capture.resize(new_capture_rows, ctx.term_width);
            let new_pinned = new_capture_rows as usize + beacon_height as usize;
            painter.set_size(ctx.term_width, ctx.term_height);
            painter = Painter::new(ctx.term_width, ctx.term_height, new_pinned);
            painter.hide_cursor();
        }

        // Drain scrollback → stream zone (scrolls into real terminal scrollback).
        for line in capture.drain_scrollback() {
            painter.stream_line(line);
        }

        state.elapsed = Some(format!("{:.1}s", start.elapsed().as_secs_f64()));

        // Build pinned frame: captured screen + beacon.
        let screen = capture.screen_lines();
        let beacon_frame = beacon::render_live(&state, start, &theme);

        let mut pinned: Vec<String> = Vec::with_capacity(screen.len() + beacon_frame.lines.len());
        pinned.extend(screen);
        pinned.extend(beacon_frame.lines.iter().cloned());

        painter.render_frame(&pinned);

        if !running {
            break;
        }

        thread::sleep(std::time::Duration::from_millis(
            theme.beacon.frame_interval_ms(),
        ));
    }

    // ─── Completion: let pulse finish, then print static beacon ──────────
    state.phase = Some("Done".into());
    state.items[0].status = StatusIcon::Success;
    state.items[0].metadata = Some(format!("{:.1}s", start.elapsed().as_secs_f64()));

    loop {
        state.elapsed = Some(format!("{:.1}s", start.elapsed().as_secs_f64()));
        let frame = beacon::render_live(&state, start, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(std::time::Duration::from_millis(
            theme.beacon.frame_interval_ms(),
        ));
        if beacon::is_at_rest(start, &theme) {
            break;
        }
    }

    state.is_active = false;
    let frame = beacon::render_static(&state, &theme);
    painter.print_final(&frame.lines);
    println!();
}
