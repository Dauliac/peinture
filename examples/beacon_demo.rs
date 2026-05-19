//! Demo: beacon component with pulse animation.
//!
//! Simulates a build pipeline: eval -> build -> done.
#![allow(clippy::print_stderr, clippy::print_stdout)]

use peinture::component::beacon::{self, BeaconState, Severity};
use peinture::component::BeaconItem;
use peinture::terminal::OutputContext;
use peinture::terminal::painter::Painter;
use peinture::tokens::Theme;
use peinture::tokens::icons::StatusIcon;
use peinture::traits::Render;
use std::time::{Duration, Instant};
use std::thread;

fn main() {
    let ctx = OutputContext::detect();
    let theme = if ctx.use_colors() { Theme::default() } else { Theme::plain() };

    println!("=== Beacon Demo ===");
    println!("TTY: {}, Colors: {}, Animations: {}", ctx.is_tty, ctx.use_colors(), ctx.use_animations());
    println!();

    if !ctx.use_pinned_region() {
        let state = demo_final_state();
        for line in &beacon::render_ci(&state, "cimera") {
            println!("{line}");
        }
        return;
    }

    let mut painter = Painter::new(ctx.term_width, ctx.term_height);
    painter.hide_cursor();

    let start = Instant::now();

    let mut state = BeaconState {
        brand: "cimera".into(),
        phase: Some("Evaluating...".into()),
        is_active: true,
        severity: Severity::Ok,
        items: vec![BeaconItem {
            status: StatusIcon::InProgress,
            message: "evaluating derivations...".into(),
            metadata: None, detail: None, priority: 10,
        }],
        ..BeaconState::default()
    };

    let crates = [
        "proc-macro2", "quote", "syn", "serde_derive", "serde", "itoa",
        "ryu", "serde_json", "libc", "memchr", "tokio-macros", "mio",
        "socket2", "tokio", "bytes", "http", "httparse", "h2", "hyper",
        "tower-service", "tower-layer", "tower", "tonic", "prost",
        "tracing-core", "tracing", "clap_lex", "clap_builder", "clap",
        "thiserror-impl", "thiserror", "uuid", "chrono", "regex-syntax",
        "regex-automata", "regex", "base64", "flate2", "rusqlite",
        "console", "glob", "blake3", "sha2", "rayon-core", "rayon",
        "notify", "fs2", "futures-core", "futures", "hyper-util",
    ];

    for i in 0..3600 {
        // Stream fake compilation lines at varied intervals
        if i % 8 == 0 && i > 0 {
            let crate_idx = (i / 8) % crates.len();
            let version = format!("{}.{}.{}", crate_idx / 10, crate_idx % 10, i % 100);
            painter.stream_line(format!(
                "   \x1b[32mCompiling\x1b[0m {} v{}", crates[crate_idx], version
            ));
        }

        let frame = beacon::render_live(&state, start, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(theme.beacon.frame_interval_ms()));

        if i == 600 {
            state.items[0].status = StatusIcon::Success;
            state.items[0].message = "evaluation completed".into();
            state.items[0].metadata = Some("2.1s".into());
            state.phase = Some("Building...".into());
            state.progress = Some("0/3 tasks".into());
            state.items.push(BeaconItem {
                status: StatusIcon::InProgress,
                message: "nix build: myservice:rust".into(),
                metadata: None, detail: None, priority: 9,
            });
        }
        if i == 1800 {
            state.items[1].status = StatusIcon::Success;
            state.items[1].message = "myservice:rust".into();
            state.items[1].metadata = Some("4.2s".into());
            state.progress = Some("2/3 tasks".into());
            state.items.push(BeaconItem {
                status: StatusIcon::Cached,
                message: "myservice:go".into(),
                metadata: Some("cached".into()),
                detail: None, priority: 5,
            });
        }
    }

    // Done — let pulse finish its beat
    state.phase = Some("Done".into());
    state.progress = Some("3 built, 1 cached".into());
    state.elapsed = Some("6.3s".into());
    state.items.push(BeaconItem {
        status: StatusIcon::Success,
        message: "cli-tools:rust".into(),
        metadata: Some("2.1s".into()),
        detail: None, priority: 8,
    });

    loop {
        let frame = beacon::render_live(&state, start, &theme);
        painter.render_frame(&frame.lines);
        thread::sleep(Duration::from_millis(theme.beacon.frame_interval_ms()));
        if beacon::is_at_rest(start, &theme) { break; }
    }

    state.is_active = false;
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
            BeaconItem { status: StatusIcon::Success, message: "evaluation completed".into(), metadata: Some("2.1s".into()), detail: None, priority: 10 },
            BeaconItem { status: StatusIcon::Success, message: "myservice:rust".into(), metadata: Some("4.2s".into()), detail: None, priority: 9 },
            BeaconItem { status: StatusIcon::Cached, message: "myservice:go".into(), metadata: Some("cached".into()), detail: None, priority: 5 },
        ],
    }
}
