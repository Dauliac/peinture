//! Demo: beacon with varied stream speeds, batch logs, pauses, notifications.
//!
//! Scenarios:
//! 1. Slow stream (1 line every ~500ms) + notifications arriving
//! 2. Fast burst (10 lines at once) + notification during burst
//! 3. Pause (no stream for 2s) + error notification with detail
//! 4. Medium stream + workload line + notification scroll-off
//! 5. Another burst + warning notification
//! 6. Slow down + build completes + pulse finish
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
    let theme = if ctx.use_colors() { Theme::default() } else { Theme::plain() };

    println!("=== Beacon Demo: Varied Speeds + Notifications ===\n");

    if !ctx.use_pinned_region() {
        println!("(CI mode — no interactive demo)");
        return;
    }

    let mut theme = theme;
    theme.beacon.notification_ttl_ms = 4_000;
    theme.beacon.notification_fade_start = 0.85;

    let min_batch = theme.beacon.max_items + 1;
    let mut painter = Painter::new(ctx.term_width, ctx.term_height, min_batch);
    painter.hide_cursor();
    let start = Instant::now();
    let frame_ms = theme.beacon.frame_interval_ms();

    let mut state = BeaconState {
        brand: "cimera".into(),
        phase: Some("Evaluating...".into()),
        is_active: true,
        severity: Severity::Ok,
        ..BeaconState::default()
    };

    // ── Scene 1: Slow stream + notifications arriving ─────────────
    // 1 line every ~500ms, notifications pop in between

    let slow_logs = [
        "evaluating attribute 'devShells.x86_64-linux.default'",
        "evaluating attribute 'packages.x86_64-linux'",
        "copying path '/nix/store/abc123-source' to remote...",
        "building '/nix/store/def456-myservice-deps-0.1.0.drv'...",
        "unpacking sources",
        "patching sources",
    ];

    for (i, log) in slow_logs.iter().enumerate() {
        painter.stream_line(format!("  {log}"));

        // Notification at log 2
        if i == 2 {
            state.push_notification(
                BeaconItem::notification(StatusIcon::Success, "nix evaluation completed").meta("6.2s")
            );
        }
        // Notification at log 4
        if i == 4 {
            state.push_notification(
                BeaconItem::notification(StatusIcon::Success, "task registry refreshed").meta("102 tasks")
            );
        }

        // Slow: render ~6 frames per log line (~500ms)
        for _ in 0..6 {
            let frame = beacon::render_live(&state, start, &theme);
            painter.render_frame(&frame.lines);
            thread::sleep(Duration::from_millis(frame_ms));
        }
    }

    // ── Scene 2: Fast burst (10 lines at once) + notification ─────
    // All 10 lines pushed in one go, then render catches up

    state.push_notification(
        BeaconItem::notification(StatusIcon::Success, "loaded hooks: rust, aws, dotenv, oci")
    );

    let burst_logs = [
        "configuring", "no configure script, doing nothing",
        "building", "running build phase",
        "compiling itoa v1.0.18", "compiling serde v1.0.200",
        "compiling tokio v1.38.0", "compiling hyper v0.14.28",
        "compiling myservice v0.1.0", "installing",
    ];
    for log in &burst_logs {
        painter.stream_line(format!("  {log}"));
    }

    // Render a few frames to flush the burst
    for _ in 0..15 {
        let frame = beacon::render_live(&state, start, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(frame_ms));
    }

    // ── Scene 3: Pause (no stream, 2s) + error notification ───────
    // Nothing streams, but an error notification pops in

    state.severity = Severity::Warning;
    state.push_notification(
        BeaconItem::notification(StatusIcon::Failed, "aws: SSO session expired")
            .detail("run: aws sso login --profile dev")
    );

    // 2 seconds of just beacon pulsing, no stream
    let pause_frames = (2000 / frame_ms) as usize;
    for _ in 0..pause_frames {
        let frame = beacon::render_live(&state, start, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(frame_ms));
    }

    // ── Scene 4: Medium stream + workload + notification scroll-off ──
    // Stream at ~200ms per line, workload appears, old notifications scroll off

    state.phase = Some("Building...".into());
    state.severity = Severity::Ok;
    state.progress = Some("0/3 tasks".into());
    state.set_workload(
        BeaconItem::workload(StatusIcon::InProgress, "nix build: myservice:rust")
    );

    let medium_logs = [
        "post-installation fixup",
        "shrinking RPATHs of ELF executables",
        "checking for references to /build/",
        "patching script interpreter paths",
        "stripping (with command strip and target flags -S -p)",
        "building '/nix/store/ghi789-myservice-0.1.0.drv'...",
        "running tests",
        "test result: ok. 42 passed; 0 failed; 0 ignored",
        "copying path '/nix/store/mno345-myservice-0.1.0' to cache...",
        "building '/nix/store/pqr012-cli-tools-0.1.0.drv'...",
    ];

    for (i, log) in medium_logs.iter().enumerate() {
        painter.stream_line(format!("  {log}"));

        // Notification at log 3: build completed
        if i == 3 {
            state.push_notification(
                BeaconItem::notification(StatusIcon::Success, "myservice:rust built").meta("12.3s")
            );
            state.set_workload(
                BeaconItem::workload(StatusIcon::InProgress, "nix build: myservice:go")
            );
            state.progress = Some("1/3 tasks".into());
        }

        // Notification at log 6: cached
        if i == 6 {
            state.push_notification(
                BeaconItem::notification(StatusIcon::Cached, "myservice:go").meta("cached")
            );
            state.set_workload(
                BeaconItem::workload(StatusIcon::InProgress, "nix build: cli-tools:rust")
            );
            state.progress = Some("2/3 tasks".into());
        }

        // Medium: ~3 frames per line (~250ms)
        for _ in 0..3 {
            let frame = beacon::render_live(&state, start, &theme);
            painter.render_frame(&frame.lines);
            thread::sleep(Duration::from_millis(frame_ms));
        }
    }

    // ── Scene 5: Another burst + warning notification ─────────────

    state.push_notification(
        BeaconItem::notification(StatusIcon::Warning, "python: virtualenv outdated")
            .detail("run: cimera sync")
    );

    let burst2 = [
        "running install phase", "running fixup phase",
        "gzip: compressed 64.2%", "running post-install hooks",
        "querying info about missing paths...",
        "downloading 'https://cache.nixos.org/nar/abc123.nar.xz'...",
    ];
    for log in &burst2 {
        painter.stream_line(format!("  {log}"));
    }

    for _ in 0..20 {
        let frame = beacon::render_live(&state, start, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(frame_ms));
    }

    // ── Scene 6: Slow wind-down + completion ──────────────────────

    let slow_end = [
        "copying 12 paths, 48.2 MiB total",
        "substituting '/nix/store/pqr678-glibc-2.39'...",
    ];

    for log in &slow_end {
        painter.stream_line(format!("  {log}"));
        for _ in 0..8 {
            let frame = beacon::render_live(&state, start, &theme);
            painter.render_frame(&frame.lines);
            thread::sleep(Duration::from_millis(frame_ms));
        }
    }

    // Build complete
    state.push_notification(
        BeaconItem::notification(StatusIcon::Success, "cli-tools:rust built").meta("8.1s")
    );
    state.clear_workload();
    state.phase = Some("Done".into());
    state.progress = Some("3 built, 1 cached".into());
    state.elapsed = Some("20.4s".into());

    // Let notifications fade and pulse finish
    let fade_frames = (5000 / frame_ms) as usize;
    for _ in 0..fade_frames {
        let frame = beacon::render_live(&state, start, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(frame_ms));
        if beacon::is_at_rest(start, &theme) && state.items.is_empty() {
            break;
        }
    }

    // Wait for pulse rest
    loop {
        let frame = beacon::render_live(&state, start, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(frame_ms));
        if beacon::is_at_rest(start, &theme) { break; }
    }

    state.is_active = false;
    let frame = beacon::render_static(&state, &theme);
    painter.print_final(&frame.lines);
    println!("\n=== Demo complete ===");
}
