//! Demo: beacon with notifications + workload scenarios.
//!
//! Simulates a full cimera session:
//! 1. DevShell entry — notifications arrive (eval, registry, hooks)
//! 2. Error notification — aws SSO expired
//! 3. Build starts — workload line appears (yellow)
//! 4. Notifications keep arriving — oldest scroll off (max 4)
//! 5. Build completes — workload clears, success notification
//! 6. Pulse finishes — static beacon printed
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

    println!("=== Beacon Demo: Notifications + Workload ===\n");

    if !ctx.use_pinned_region() {
        let state = demo_final_state();
        for line in &beacon::render_ci(&state, "cimera") {
            println!("{line}");
        }
        return;
    }

    // Fast fade for demo
    let mut theme = theme;
    theme.beacon.notification_ttl_ms = 3_000;
    theme.beacon.notification_fade_start = 0.80;

    let min_batch = theme.beacon.max_items + 1; // batch fills any notification gap
    let mut painter = Painter::new(ctx.term_width, ctx.term_height, min_batch);
    painter.hide_cursor();
    let start = Instant::now();
    let frame_ms = theme.beacon.frame_interval_ms();

    let mut state = BeaconState {
        brand: "cimera".into(),
        is_active: true,
        severity: Severity::Ok,
        ..BeaconState::default()
    };

    // ── Phase 1: DevShell notifications ──────────────────────────

    // Notification: eval started
    state.phase = Some("Evaluating...".into());
    render_frames(&mut painter, &state, start, &theme, frame_ms, 30);

    // Notification: eval completed
    state.push_notification(
        BeaconItem::notification(StatusIcon::Success, "nix evaluation completed").meta("6.2s")
    );
    render_frames(&mut painter, &state, start, &theme, frame_ms, 25);

    // Notification: registry refreshed
    state.push_notification(
        BeaconItem::notification(StatusIcon::Success, "task registry refreshed").meta("102 tasks")
    );
    render_frames(&mut painter, &state, start, &theme, frame_ms, 25);

    // ── Phase 2: Error notification ──────────────────────────────

    // Notification: AWS error (red)
    state.severity = Severity::Warning;
    state.push_notification(
        BeaconItem::notification(StatusIcon::Failed, "aws: SSO session expired")
            .detail("run: aws sso login --profile dev")
    );
    render_frames(&mut painter, &state, start, &theme, frame_ms, 30);

    // Notification: hooks loaded (success)
    state.push_notification(
        BeaconItem::notification(StatusIcon::Success, "loaded hooks: rust, aws, dotenv, oci")
    );
    render_frames(&mut painter, &state, start, &theme, frame_ms, 25);

    // ── Phase 3: Build starts — workload line appears ────────────

    state.phase = Some("Building...".into());
    state.severity = Severity::Ok;
    state.set_workload(
        BeaconItem::workload(StatusIcon::InProgress, "nix build: myservice:rust")
    );
    state.progress = Some("0/3 tasks".into());

    render_frames(&mut painter, &state, start, &theme, frame_ms, 40);

    // ── Phase 4: More notifications push in — oldest scroll off ──

    // 5th notification: should cause the 1st (eval completed) to scroll off
    state.push_notification(
        BeaconItem::notification(StatusIcon::Success, "myservice:rust built").meta("12.3s")
    );
    state.set_workload(
        BeaconItem::workload(StatusIcon::InProgress, "nix build: myservice:go")
    );
    state.progress = Some("1/3 tasks".into());
    render_frames(&mut painter, &state, start, &theme, frame_ms, 30);

    // 6th notification: another one scrolls off
    state.push_notification(
        BeaconItem::notification(StatusIcon::Cached, "myservice:go").meta("cached")
    );
    state.set_workload(
        BeaconItem::workload(StatusIcon::InProgress, "nix build: cli-tools:rust")
    );
    state.progress = Some("2/3 tasks".into());
    render_frames(&mut painter, &state, start, &theme, frame_ms, 30);

    // Warning notification
    state.push_notification(
        BeaconItem::notification(StatusIcon::Warning, "python: virtualenv outdated")
            .detail("run: cimera sync")
    );
    render_frames(&mut painter, &state, start, &theme, frame_ms, 30);

    // ── Phase 5: Build completes ─────────────────────────────────

    state.push_notification(
        BeaconItem::notification(StatusIcon::Success, "cli-tools:rust built").meta("8.1s")
    );
    state.clear_workload();
    state.phase = Some("Done".into());
    state.progress = Some("3 built, 1 cached".into());
    state.elapsed = Some("20.4s".into());

    // Let pulse finish
    loop {
        let frame = beacon::render_live(&state, start, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(frame_ms));
        if beacon::is_at_rest(start, &theme) { break; }
    }

    // ── Final static beacon ──────────────────────────────────────

    state.is_active = false;
    let frame = beacon::render_static(&state, &theme);
    painter.print_final(&frame.lines);
    println!("\n=== Demo complete ===");
}

static LOG_LINES: &[&str] = &[
    "evaluating attribute 'devShells.x86_64-linux.default'",
    "copying path '/nix/store/abc123-source' to remote...",
    "building '/nix/store/def456-myservice-deps-0.1.0.drv'...",
    "unpacking sources",
    "patching sources",
    "running build phase",
    "installing",
    "post-installation fixup",
    "shrinking RPATHs of ELF executables",
    "building '/nix/store/ghi789-myservice-0.1.0.drv'...",
    "running tests",
    "test result: ok. 42 passed; 0 failed",
    "copying path '/nix/store/jkl012-myservice-0.1.0' to cache...",
    "building '/nix/store/mno345-cli-tools-0.1.0.drv'...",
    "running install phase",
    "gzip: compressed 64.2%",
    "querying info about missing paths...",
    "downloading 'https://cache.nixos.org/nar/abc123.nar.xz'...",
    "copying 12 paths, 48.2 MiB total",
    "substituting '/nix/store/pqr678-glibc-2.39'...",
];

/// Global counter for cycling through log lines.
static LOG_IDX: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn render_frames(
    painter: &mut Painter,
    state: &BeaconState,
    start: Instant,
    theme: &Theme,
    frame_ms: u64,
    count: usize,
) {
    for i in 0..count {
        // Stream a log line every 3 frames
        if i % 3 == 0 {
            let idx = LOG_IDX.fetch_add(1, std::sync::atomic::Ordering::Relaxed) % LOG_LINES.len();
            painter.stream_line(format!("  {}", LOG_LINES[idx]));
        }
        let frame = beacon::render_live(state, start, theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(frame_ms));
    }
}

fn demo_final_state() -> BeaconState {
    let mut state = BeaconState {
        brand: "cimera".into(),
        phase: Some("Done".into()),
        progress: Some("3 built, 1 cached".into()),
        elapsed: Some("20.4s".into()),
        severity: Severity::Ok,
        is_active: false,
        ..BeaconState::default()
    };
    state.push_notification(BeaconItem::notification(StatusIcon::Success, "cli-tools:rust built").meta("8.1s"));
    state.push_notification(BeaconItem::notification(StatusIcon::Cached, "myservice:go").meta("cached"));
    state.push_notification(BeaconItem::notification(StatusIcon::Success, "myservice:rust built").meta("12.3s"));
    state
}
