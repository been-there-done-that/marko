use rune::{Env, Runtime};

use super::*;
use crate::app::AppState;
use crate::ui::view;

fn runtime(state: AppState) -> Runtime<AppState> {
    Runtime::new(state, view, (1100.0, 760.0), Env { system_dark: false, scale: 2.0 })
}

/// Lays the view out, then presses the element with `key` and lets the frame settle.
fn press(rt: &mut Runtime<AppState>, key: &str) {
    rt.frame(1.0 / 60.0);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element `{key}`"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..3 {
        rt.frame(1.0 / 60.0);
    }
}

/// A reading of `rss` bytes and `cpu_s` seconds of CPU, as `procwatch` reports them.
fn usage(rss: u64, cpu_s: f64) -> Option<(u64, f64)> {
    Some((rss, cpu_s))
}

#[test]
fn stats_text_formats() {
    assert_eq!(text(Stats { rss_mb: 112, cpu_pct: 0 }), "112 MB · 0% CPU");
    assert_eq!(text(Stats { rss_mb: 1024, cpu_pct: 37 }), "1024 MB · 37% CPU");
}

#[test]
fn cpu_is_taken_over_wall_time_since_the_last_sample() {
    let (first, sample) = compute(None, 0.0, 112 << 20, 2.0);
    assert_eq!(first, Stats { rss_mb: 112, cpu_pct: 0 });
    // A quarter of a core over two seconds: 0.5 s of CPU out of 2 s of wall time.
    let (next, _) = compute(Some(sample), 2.0, 112 << 20, 2.5);
    assert_eq!(next.cpu_pct, 25);
}

#[test]
fn apply_samples_once_per_second_and_reports_only_changes() {
    let mut s = AppState::new();
    assert!(apply(&mut s, 0.0, usage(112 << 20, 1.0)), "the first reading shows");
    assert!(!apply(&mut s, 0.5, usage(112 << 20, 1.1)), "not due yet");
    assert_eq!(s.stats.cpu_pct, 0);
    assert!(!apply(&mut s, 1.0, usage(112 << 20, 1.0)), "same text, no redraw");
    assert!(apply(&mut s, 2.0, usage(120 << 20, 1.5)), "memory moved");
    assert_eq!(s.stats, Stats { rss_mb: 120, cpu_pct: 50 });
}

#[test]
fn stats_hidden_when_toggled_off() {
    let mut s = AppState::new();
    apply(&mut s, 0.0, usage(112 << 20, 1.0));
    let mut rt = runtime(s);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("stats").is_some(), "the readout shows by default");
    press(&mut rt, "tb-more");
    press(&mut rt, "menu-stats");
    assert!(!rt.state.stats_on);
    assert_eq!(rt.state.menu, None);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("stats").is_none());
}
