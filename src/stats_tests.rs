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
fn memory_counts_decimal_megabytes_as_the_system_monitor_does() {
    // 84 MiB is 88.08 MB: the figure GNOME's monitors show for the same process.
    let (stats, _) = compute(None, 0.0, 84 << 20, 0.0);
    assert_eq!(stats.rss_mb, 88);
}

#[test]
fn cpu_is_taken_over_wall_time_since_the_last_sample() {
    let (first, sample) = compute(None, 0.0, 112_000_000, 2.0);
    assert_eq!(first, Stats { rss_mb: 112, cpu_pct: 0 });
    // A quarter of a core over two seconds: 0.5 s of CPU out of 2 s of wall time.
    let (next, _) = compute(Some(sample), 2.0, 112_000_000, 2.5);
    assert_eq!(next.cpu_pct, 25);
}

#[test]
fn apply_samples_once_per_second_and_reports_only_changes() {
    let mut s = AppState::new();
    assert!(apply(&mut s, 0.0, usage(112_000_000, 1.0)), "the first reading shows");
    assert!(!apply(&mut s, 0.5, usage(112_000_000, 1.1)), "not due yet");
    assert_eq!(s.stats.cpu_pct, 0);
    assert!(!apply(&mut s, 1.0, usage(112_000_000, 1.0)), "same text, no redraw");
    assert!(apply(&mut s, 2.0, usage(120_000_000, 1.5)), "memory moved");
    assert_eq!(s.stats, Stats { rss_mb: 120, cpu_pct: 50 });
}

#[test]
fn stats_hidden_when_toggled_off() {
    let mut s = AppState::new();
    apply(&mut s, 0.0, usage(112_000_000, 1.0));
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

#[test]
fn parses_proc_files() {
    let statm = "12345 2560 300 1 0 900 0\n";
    let stat = "4242 (marko (x)) S 1 2 3 4 5 6 7 8 9 10 150 50 0 0 20 0 9 0 100\n";
    let (rss, cpu) = parse_proc(statm, stat, 4096, 100.0).unwrap();
    assert_eq!(rss, 2560 * 4096);
    assert!((cpu - 2.0).abs() < 1e-9, "150 + 50 ticks at 100 Hz is 2 s, got {cpu}");
}

#[cfg(target_os = "linux")]
#[test]
fn reads_this_process_on_linux() {
    let (rss, _) = read_self().expect("/proc/self is readable");
    assert!(rss > 1 << 20, "a test process uses over 1 MB");
}
