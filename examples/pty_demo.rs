//! Demo: capture a screen-rewriting program via PTY and overlay a peinture
//! beacon below it.
//!
//! The beacon follows the captured content — it starts near the top and
//! moves down as the program fills the screen, just like `beacon_demo`.
//!
//! Layout (grows over time):
//!   ─── top of terminal ───
//!   captured program screen     ← only active (non-empty) rows
//!   beacon                      ← right below, moves down as content grows
//!   (empty space)
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
use peinture::terminal::pty_capture::PtyCapture;
use peinture::tokens::Theme;
use peinture::tokens::icons::StatusIcon;
use std::io::Write;
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

    let mut ctx = OutputContext::detect();
    if !ctx.use_pinned_region() {
        eprintln!("This demo requires an interactive terminal (TTY).");
        std::process::exit(1);
    }

    let theme = if ctx.use_colors() { Theme::default() } else { Theme::plain() };
    let cmd_display = args.join(" ");
    let frame_ms = theme.beacon.frame_interval_ms();

    let mut state = BeaconState {
        brand: "cimera".into(),
        phase: Some("Running...".into()),
        is_active: true,
        severity: Severity::Ok,
        ..BeaconState::default()
    };
    state.set_workload(
        BeaconItem::workload(StatusIcon::InProgress, &cmd_display),
    );

    // Reserve space for beacon at max capacity.
    let probe_state = beacon_probe_state(&theme);
    let beacon_height = beacon::render_live(&probe_state, Instant::now(), &theme)
        .lines
        .len() as u16;
    let capture_rows = ctx.term_height.saturating_sub(beacon_height).max(4);

    let str_args: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
    let mut capture = PtyCapture::spawn(&args[0], &str_args, capture_rows, ctx.term_width)
        .unwrap_or_else(|e| {
            eprintln!("Failed to spawn '{}': {e}", args[0]);
            std::process::exit(1);
        });

    let _ = std::io::stderr().write_all(b"\x1b[?25l");
    let _ = std::io::stderr().flush();
    install_cleanup_hook();

    let start = Instant::now();

    // ─── Main render loop ────────────────────────────────────────────────
    loop {
        let running = capture.process_available();

        // Handle terminal resize.
        let prev_width = ctx.term_width;
        let prev_height = ctx.term_height;
        ctx.refresh_size();
        if ctx.term_width != prev_width || ctx.term_height != prev_height {
            let new_capture_rows = ctx.term_height.saturating_sub(beacon_height).max(4);
            capture.resize(new_capture_rows, ctx.term_width);
        }

        state.elapsed = Some(format!("{:.1}s", start.elapsed().as_secs_f64()));

        // Only render active (non-empty) rows — beacon follows the content.
        let screen = capture.screen_lines();
        let active = capture.active_rows();
        let active_screen = &screen[..active];
        let beacon_frame = beacon::render_live(&state, start, &theme);

        render_compact(active_screen, &beacon_frame.lines, ctx.term_height);

        if !running {
            break;
        }

        thread::sleep(Duration::from_millis(frame_ms));
    }

    // ─── Completion ──────────────────────────────────────────────────────
    state.clear_workload();
    state.phase = Some("Done".into());
    state.push_notification(
        BeaconItem::notification(StatusIcon::Success, &cmd_display)
            .meta(format!("{:.1}s", start.elapsed().as_secs_f64())),
    );

    loop {
        state.elapsed = Some(format!("{:.1}s", start.elapsed().as_secs_f64()));
        let beacon_frame = beacon::render_live(&state, start, &theme);
        render_compact(&[], &beacon_frame.lines, ctx.term_height);
        thread::sleep(Duration::from_millis(frame_ms));
        if beacon::is_at_rest(start, &theme) {
            break;
        }
    }

    // ─── Final: clear screen, print static beacon, restore cursor ────────
    state.is_active = false;
    let frame = beacon::render_static(&state, &theme);

    let mut buf = String::new();
    buf.push_str("\x1b[r\x1b[2J\x1b[1;1H");
    for line in &frame.lines {
        buf.push_str(line);
        buf.push('\n');
    }
    buf.push_str("\x1b[?25h");
    let _ = std::io::stderr().write_all(buf.as_bytes());
    let _ = std::io::stderr().flush();
}

/// Render content + beacon as a compact block starting from row 1.
/// The beacon sits right below the active content and moves down as
/// the content grows — empty space stays below.
fn render_compact(screen_lines: &[String], beacon_lines: &[String], term_height: u16) {
    let mut buf = String::with_capacity(4096);
    buf.push_str("\x1b[?2026h");

    let mut row: u16 = 1;

    // Active captured screen lines.
    for line in screen_lines {
        if row > term_height {
            break;
        }
        buf.push_str(&format!("\x1b[{row};1H\x1b[2K{line}"));
        row += 1;
    }

    // Beacon right below.
    for line in beacon_lines {
        if row > term_height {
            break;
        }
        buf.push_str(&format!("\x1b[{row};1H\x1b[2K{line}"));
        row += 1;
    }

    // Clear remaining rows below.
    while row <= term_height {
        buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
        row += 1;
    }

    buf.push_str(&format!("\x1b[{term_height};1H"));
    buf.push_str("\x1b[?2026l");

    let _ = std::io::stderr().write_all(buf.as_bytes());
    let _ = std::io::stderr().flush();
}

/// Build a probe state with max items to measure worst-case beacon height.
fn beacon_probe_state(theme: &Theme) -> BeaconState {
    let mut s = BeaconState {
        brand: "cimera".into(),
        phase: Some("x".into()),
        progress: Some("x".into()),
        elapsed: Some("x".into()),
        is_active: true,
        severity: Severity::Ok,
        ..BeaconState::default()
    };
    for _ in 0..theme.beacon.max_items {
        s.push_notification(BeaconItem::notification(StatusIcon::InProgress, "x"));
    }
    s.set_workload(BeaconItem::workload(StatusIcon::InProgress, "x"));
    s
}

fn install_cleanup_hook() {
    let _ = ctrlc::set_handler(move || {
        let _ = std::io::stderr().write_all(b"\x1b[r\x1b[?25h\x1b[2J\x1b[1;1H");
        let _ = std::io::stderr().flush();
        std::process::exit(130);
    });
}
