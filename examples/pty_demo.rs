//! Demo: capture a screen-rewriting program via PTY and overlay a peinture
//! beacon below it using a scroll region.
//!
//! The virtual terminal starts small and grows reactively as the captured
//! program fills it. This pushes the beacon down organically — just like
//! `beacon_demo` where stream lines push the beacon to the bottom.
//!
//! Once the virtual terminal reaches full size, content scrolls naturally
//! into real terminal scrollback.
//!
//! Usage:
//!   cargo run --features pty --example pty_demo -- <command> [args...]
//!
//! Examples:
//!   cargo run --features pty --example pty_demo -- \
//!       nom -- nix build '.#devShells.x86_64-linux.default'
//!
//!   cargo run --features pty --example pty_demo -- htop
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

    // Measure beacon height at max capacity.
    let probe = beacon_probe_state(&theme);
    let beacon_height = beacon::render_live(&probe, Instant::now(), &theme)
        .lines
        .len() as u16;

    let max_capture_rows = ctx.term_height.saturating_sub(beacon_height).max(4);
    // Start at 1 row — the PTY grows as the program fills it.
    let mut current_rows: u16 = 1;

    let str_args: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
    let mut capture = PtyCapture::spawn(&args[0], &str_args, current_rows, ctx.term_width)
        .unwrap_or_else(|e| {
            eprintln!("Failed to spawn '{}': {e}", args[0]);
            std::process::exit(1);
        });

    let mut stderr = std::io::stderr();

    // Hide cursor, set initial scroll region, position cursor at top.
    let _ = stderr.write_all(
        format!("\x1b[?25l\x1b[1;{current_rows}r\x1b[1;1H").as_bytes(),
    );
    let _ = stderr.flush();
    install_cleanup_hook();

    let start = Instant::now();

    // ─── Main render loop ────────────────────────────────────────────────
    loop {
        let (running, raw) = capture.process_available_raw();

        // Relay nom's raw output into the scroll region.
        if !raw.is_empty() {
            let _ = stderr.write_all(&raw);
            let _ = stderr.flush();
        }

        // Grow the virtual terminal when the program needs more space.
        if capture.needs_grow() && current_rows < max_capture_rows {
            // Grow by a few rows to reduce SIGWINCH frequency.
            let new_rows = (current_rows + 4).min(max_capture_rows);
            current_rows = new_rows;
            capture.resize(current_rows, ctx.term_width);
            // Expand scroll region to match.
            let _ = stderr.write_all(
                format!("\x1b[1;{current_rows}r").as_bytes(),
            );
            let _ = stderr.flush();
        }

        // Handle real terminal resize.
        let prev_width = ctx.term_width;
        let prev_height = ctx.term_height;
        ctx.refresh_size();
        if ctx.term_width != prev_width || ctx.term_height != prev_height {
            let new_max = ctx.term_height.saturating_sub(beacon_height).max(4);
            current_rows = current_rows.min(new_max);
            capture.resize(current_rows, ctx.term_width);
            let _ = stderr.write_all(
                format!("\x1b[1;{current_rows}r").as_bytes(),
            );
            let _ = stderr.flush();
        }

        // Draw beacon below the scroll region.
        state.elapsed = Some(format!("{:.1}s", start.elapsed().as_secs_f64()));
        let beacon_frame = beacon::render_live(&state, start, &theme);
        draw_beacon(&beacon_frame.lines, current_rows, ctx.term_height);

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
        draw_beacon(&beacon_frame.lines, current_rows, ctx.term_height);
        thread::sleep(Duration::from_millis(frame_ms));
        if beacon::is_at_rest(start, &theme) {
            break;
        }
    }

    // ─── Final: reset scroll region, print static beacon ─────────────────
    state.is_active = false;
    let frame = beacon::render_static(&state, &theme);

    let mut buf = String::new();
    buf.push_str("\x1b[r"); // reset scroll region
    let final_row = current_rows + 1;
    buf.push_str(&format!("\x1b[{final_row};1H\x1b[J")); // move below nom, clear rest
    for line in &frame.lines {
        buf.push_str(line);
        buf.push('\n');
    }
    buf.push_str("\x1b[?25h");
    let _ = std::io::stderr().write_all(buf.as_bytes());
    let _ = std::io::stderr().flush();
}

/// Draw beacon below the scroll region using save/restore cursor.
fn draw_beacon(lines: &[String], scroll_bottom: u16, term_height: u16) {
    let mut buf = String::with_capacity(2048);
    buf.push_str("\x1b[?2026h"); // sync begin
    buf.push_str("\x1b7");       // save cursor (DECSC)

    let beacon_start = scroll_bottom + 1;
    for (i, line) in lines.iter().enumerate() {
        let row = beacon_start + i as u16;
        if row > term_height {
            break;
        }
        buf.push_str(&format!("\x1b[{row};1H\x1b[2K{line}"));
    }
    // Clear any leftover rows between beacon and bottom.
    let beacon_end = beacon_start + lines.len() as u16;
    for row in beacon_end..=term_height {
        buf.push_str(&format!("\x1b[{row};1H\x1b[2K"));
    }

    buf.push_str("\x1b8");       // restore cursor (DECRC)
    buf.push_str("\x1b[?2026l"); // sync end

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
        let _ = std::io::stderr().write_all(b"\x1b[r\x1b[?25h");
        let _ = std::io::stderr().flush();
        std::process::exit(130);
    });
}
