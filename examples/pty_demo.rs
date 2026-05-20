//! Demo: capture a screen-rewriting program via PTY and overlay a peinture
//! beacon below it using a scroll region.
//!
//! nom's raw output is relayed directly into a DECSTBM scroll region.
//! The beacon is drawn below the scroll region via absolute positioning.
//! When content scrolls off the top, it goes into real terminal scrollback.
//!
//! Layout:
//!   ─── top of terminal ───
//!   scroll region (rows 1 to N)   ← nom's raw output relayed here
//!   beacon (rows N+1 to bottom)   ← redrawn each frame via save/restore cursor
//!   ─── bottom of terminal ───
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

    // Scroll region = rows 1..scroll_bottom. Beacon lives below.
    let scroll_bottom = ctx.term_height.saturating_sub(beacon_height);
    let capture_rows = scroll_bottom.max(4);

    let str_args: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();
    let mut capture = PtyCapture::spawn(&args[0], &str_args, capture_rows, ctx.term_width)
        .unwrap_or_else(|e| {
            eprintln!("Failed to spawn '{}': {e}", args[0]);
            std::process::exit(1);
        });

    let mut stderr = std::io::stderr();

    // Hide cursor, set scroll region, position cursor at top.
    let _ = stderr.write_all(
        format!("\x1b[?25l\x1b[1;{scroll_bottom}r\x1b[1;1H").as_bytes(),
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

        // Handle terminal resize.
        let prev_width = ctx.term_width;
        let prev_height = ctx.term_height;
        ctx.refresh_size();
        if ctx.term_width != prev_width || ctx.term_height != prev_height {
            let new_scroll_bottom = ctx.term_height.saturating_sub(beacon_height);
            let new_capture_rows = new_scroll_bottom.max(4);
            capture.resize(new_capture_rows, ctx.term_width);
            // Update scroll region.
            let _ = stderr.write_all(
                format!("\x1b[1;{new_scroll_bottom}r").as_bytes(),
            );
            let _ = stderr.flush();
        }

        // Draw beacon below the scroll region (save/restore cursor so
        // nom's cursor position is preserved).
        state.elapsed = Some(format!("{:.1}s", start.elapsed().as_secs_f64()));
        let beacon_frame = beacon::render_live(&state, start, &theme);
        draw_beacon(
            &beacon_frame.lines,
            ctx.term_height.saturating_sub(beacon_height),
            ctx.term_height,
        );

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

    // Let pulse finish.
    loop {
        state.elapsed = Some(format!("{:.1}s", start.elapsed().as_secs_f64()));
        let beacon_frame = beacon::render_live(&state, start, &theme);
        draw_beacon(
            &beacon_frame.lines,
            ctx.term_height.saturating_sub(beacon_height),
            ctx.term_height,
        );
        thread::sleep(Duration::from_millis(frame_ms));
        if beacon::is_at_rest(start, &theme) {
            break;
        }
    }

    // ─── Final: reset scroll region, clear beacon area, print static ─────
    state.is_active = false;
    let frame = beacon::render_static(&state, &theme);

    let mut buf = String::new();
    // Reset scroll region, move below last nom output, show cursor.
    buf.push_str("\x1b[r");
    // Move to the row after the scroll region to print final beacon.
    let final_row = ctx.term_height.saturating_sub(beacon_height) + 1;
    buf.push_str(&format!("\x1b[{final_row};1H"));
    // Clear from here to bottom.
    buf.push_str("\x1b[J");
    for line in &frame.lines {
        buf.push_str(line);
        buf.push('\n');
    }
    buf.push_str("\x1b[?25h");
    let _ = std::io::stderr().write_all(buf.as_bytes());
    let _ = std::io::stderr().flush();
}

/// Draw beacon below the scroll region using save/restore cursor.
/// This preserves nom's cursor position so its output isn't disrupted.
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
        // Reset scroll region + show cursor.
        let _ = std::io::stderr().write_all(b"\x1b[r\x1b[?25h");
        let _ = std::io::stderr().flush();
        std::process::exit(130);
    });
}
