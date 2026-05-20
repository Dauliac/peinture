//! Demo: capture a screen-rewriting program via PTY and overlay a peinture
//! beacon below it.
//!
//! Layout:
//!   ─── top of terminal ───
//!   captured program screen     ← vt100 virtual terminal, redrawn in place
//!   beacon                      ← peinture beacon, redrawn in place
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

    // Compute beacon height to size the virtual terminal.
    // Use max_items + 1 to reserve space for a full notification stack.
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

    // Hide cursor and install ctrlc handler to restore it.
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

        let screen = capture.screen_lines();
        let beacon_frame = beacon::render_live(&state, start, &theme);
        render_fullscreen(&screen, &beacon_frame.lines, ctx.term_height);

        if !running {
            break;
        }

        thread::sleep(Duration::from_millis(frame_ms));
    }

    // ─── Completion: workload done, push success notification ────────────
    state.clear_workload();
    state.phase = Some("Done".into());
    state.push_notification(
        BeaconItem::notification(StatusIcon::Success, &cmd_display)
            .meta(format!("{:.1}s", start.elapsed().as_secs_f64())),
    );

    // Let pulse finish its cycle.
    loop {
        state.elapsed = Some(format!("{:.1}s", start.elapsed().as_secs_f64()));
        let beacon_frame = beacon::render_live(&state, start, &theme);
        render_fullscreen(&[], &beacon_frame.lines, ctx.term_height);
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

/// Render the full terminal: captured screen at the top, beacon at the bottom.
/// Uses synchronized updates and absolute cursor positioning — no scroll region.
fn render_fullscreen(screen_lines: &[String], beacon_lines: &[String], term_height: u16) {
    let mut buf = String::with_capacity(4096);

    // Synchronized update: hold display until frame is complete.
    buf.push_str("\x1b[?2026h");

    // Draw captured screen lines starting at row 1.
    for (i, line) in screen_lines.iter().enumerate() {
        let row = i as u16 + 1;
        if row > term_height {
            break;
        }
        buf.push_str(&format!("\x1b[{row};1H\x1b[2K{line}"));
    }

    // Draw beacon lines at the bottom.
    let beacon_start = term_height.saturating_sub(beacon_lines.len() as u16) + 1;
    for (i, line) in beacon_lines.iter().enumerate() {
        let row = beacon_start + i as u16;
        if row > term_height {
            break;
        }
        buf.push_str(&format!("\x1b[{row};1H\x1b[2K{line}"));
    }

    // Clear any gap between screen and beacon.
    let screen_end = screen_lines.len() as u16 + 1;
    for row in screen_end..beacon_start {
        buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
    }

    // Park cursor out of the way.
    buf.push_str(&format!("\x1b[{term_height};1H"));

    // End synchronized update.
    buf.push_str("\x1b[?2026l");

    let _ = std::io::stderr().write_all(buf.as_bytes());
    let _ = std::io::stderr().flush();
}

fn install_cleanup_hook() {
    let _ = ctrlc::set_handler(move || {
        let _ = std::io::stderr().write_all(b"\x1b[r\x1b[?25h\x1b[2J\x1b[1;1H");
        let _ = std::io::stderr().flush();
        std::process::exit(130);
    });
}
