//! Demo: beacon component with pulse animation and updating items.
//!
//! Simulates a build pipeline: eval -> build -> done.
//! The beacon is pinned at the bottom while "build output" streams above.
#![allow(clippy::print_stderr, clippy::print_stdout)]

use peinture::component::beacon::{self, BeaconState, Severity, render_ci};
use peinture::component::pulse::Pulse;
use peinture::component::BeaconItem;
use peinture::terminal::OutputContext;
use peinture::terminal::painter::Painter;
use peinture::tokens::Theme;
use peinture::tokens::icons::StatusIcon;
use std::thread;
use std::time::Duration;

fn main() {
    let ctx = OutputContext::detect();
    let theme = if ctx.use_colors() {
        Theme::default()
    } else {
        Theme::plain()
    };

    println!("=== Beacon Demo ===");
    println!("TTY: {}, Colors: {}, Animations: {}", ctx.is_tty, ctx.use_colors(), ctx.use_animations());
    println!();

    if !ctx.use_pinned_region() {
        // CI/pipe mode: just print static beacon
        let state = demo_final_state();
        let lines = render_ci(&state, "cimera");
        for line in &lines {
            println!("{line}");
        }
        return;
    }

    // Interactive mode with pinned beacon
    let mut painter = Painter::new(ctx.term_width, ctx.term_height);
    painter.hide_cursor();

    // Single pulse — lives for the entire animation
    let pulse = Pulse::new();

    // Phase 1: Evaluating
    let mut state = BeaconState {
        brand: "cimera".into(),
        phase: Some("Evaluating...".into()),
        is_active: true,
        severity: Severity::Ok,
        items: vec![BeaconItem {
            status: StatusIcon::InProgress,
            message: "evaluating derivations...".into(),
            metadata: None,
            detail: None,
            priority: 10,
        }],
        ..BeaconState::default()
    };

    for i in 0..25 {
        // Simulate build output streaming above beacon
        if i % 3 == 0 && i > 0 {
            painter.stream_line(format!("  | Compiling dep-{} v0.1.0", i / 3));
        }

        // Render with the SAME pulse instance — elapsed time accumulates
        let frame = beacon::render_live(&state, &pulse, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(theme.beacon.frame_interval_ms()));

        // Update state as time passes
        if i == 8 {
            state.items[0].status = StatusIcon::Success;
            state.items[0].message = "evaluation completed".into();
            state.items[0].metadata = Some("2.1s".into());
            state.phase = Some("Building...".into());
            state.progress = Some("0/3 tasks".into());
            state.items.push(BeaconItem {
                status: StatusIcon::InProgress,
                message: "nix build: myservice:rust".into(),
                metadata: None,
                detail: None,
                priority: 9,
            });
        }
        if i == 16 {
            state.items[1].status = StatusIcon::Success;
            state.items[1].message = "myservice:rust".into();
            state.items[1].metadata = Some("4.2s".into());
            state.progress = Some("1/3 tasks".into());
            state.items.push(BeaconItem {
                status: StatusIcon::Cached,
                message: "myservice:go".into(),
                metadata: Some("cached".into()),
                detail: None,
                priority: 5,
            });
            state.progress = Some("2/3 tasks".into());
        }
    }

    // Phase 3: Done
    state.phase = Some("Done".into());
    state.progress = Some("3 built, 1 cached".into());
    state.elapsed = Some("6.3s".into());
    state.is_active = false;
    state.items.push(BeaconItem {
        status: StatusIcon::Success,
        message: "cli-tools:rust".into(),
        metadata: Some("2.1s".into()),
        detail: None,
        priority: 8,
    });

    // Final static frame (no pulse)
    let frame = beacon::render_static(&state, &theme);
    painter.print_final(&frame.lines);

    println!("\n=== Demo complete ===");
}

fn demo_final_state() -> BeaconState {
    BeaconState {
        brand: "cimera".into(),
        phase: Some("Done".into()),
        progress: Some("3 built, 1 cached".into()),
        elapsed: Some("6.3s".into()),
        severity: Severity::Ok,
        is_active: false,
        items: vec![
            BeaconItem {
                status: StatusIcon::Success,
                message: "evaluation completed".into(),
                metadata: Some("2.1s".into()),
                detail: None,
                priority: 10,
            },
            BeaconItem {
                status: StatusIcon::Success,
                message: "myservice:rust".into(),
                metadata: Some("4.2s".into()),
                detail: None,
                priority: 9,
            },
            BeaconItem {
                status: StatusIcon::Cached,
                message: "myservice:go".into(),
                metadata: Some("cached".into()),
                detail: None,
                priority: 5,
            },
        ],
    }
}
