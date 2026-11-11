//! This process's memory and CPU, sampled once a second for the titlebar readout. The numbers
//! come from `/proc/self` on Linux and `procwatch` elsewhere.

use std::time::Duration;

use rune::Tx;

use crate::app::AppState;

/// How often the readout samples, in seconds.
pub const EVERY: f64 = 1.0;

/// What the readout shows: resident memory in whole MB (1 MB = 1,000,000 bytes, as GNOME's and
/// macOS's monitors count), and CPU use since the last sample in whole percent of one core.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub rss_mb: u32,
    pub cpu_pct: u32,
}

/// A reading of the process's CPU time at `at` seconds, the base the next one is taken from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub at: f64,
    pub cpu_s: f64,
}

/// The readout's text: `112 MB · 0% CPU`.
pub fn text(stats: Stats) -> String {
    format!("{} MB · {}% CPU", stats.rss_mb, stats.cpu_pct)
}

/// The numbers for a reading at `now` against the previous sample, and the sample to diff the
/// next one against. CPU is over the wall time since `prev`; with no `prev` it is 0.
pub fn compute(prev: Option<Sample>, now: f64, rss_bytes: u64, cpu_s: f64) -> (Stats, Sample) {
    let rss_mb = (rss_bytes as f64 / 1e6).round() as u32;
    let cpu_pct = match prev {
        Some(p) if now > p.at => ((cpu_s - p.cpu_s).max(0.0) / (now - p.at) * 100.0).round() as u32,
        _ => 0,
    };
    (Stats { rss_mb, cpu_pct }, Sample { at: now, cpu_s })
}

/// Takes a reading of `(resident bytes, CPU seconds)` if a sample is due at `now`. Returns true
/// when the shown text changed, so `poll` redraws only then. No reading leaves the state as it was.
pub fn apply(s: &mut AppState, now: f64, reading: Option<(u64, f64)>) -> bool {
    let Some((rss, cpu_s)) = reading else { return false };
    if s.stats_last.is_some_and(|prev| now - prev.at < EVERY - 0.02) {
        return false;
    }
    let first = s.stats_last.is_none();
    let (stats, sample) = compute(s.stats_last, now, rss, cpu_s);
    s.stats_last = Some(sample);
    let changed = first || stats != s.stats;
    s.stats = stats;
    changed
}

/// Once per poll: samples the process when the readout is on and a sample is due, and asks for
/// the next wake-up at the next sample time, so the numbers keep moving while the window is idle.
/// Returns true only when the shown text changed.
pub fn tick(s: &mut AppState, now: f64, tx: &mut Tx) -> bool {
    if !s.stats_on {
        s.stats_last = None;
        return false;
    }
    let due = s.stats_last.is_none_or(|prev| now - prev.at >= EVERY - 0.02);
    let first = s.stats_last.is_none();
    let changed = due && apply(s, now, read_self());
    if changed {
        // The number goes into a live text, so the window repaints that text without rebuilding
        // the view: a rebuild of a large document costs CPU the next sample would then report.
        s.stats_text.set(text(s.stats));
    }
    if let Some(prev) = s.stats_last {
        tx.wake_after(Duration::from_secs_f64((prev.at + EVERY - now).max(0.0)));
    }
    // Only the first reading changes the view's shape (the readout appears).
    first && changed
}

/// `(resident bytes, CPU seconds)` for this process. On Linux it reads `/proc/self`, since the
/// pinned `procwatch` rev reads nothing off macOS. Both files are kernel-generated and tiny.
#[cfg(target_os = "linux")]
pub fn read_self() -> Option<(u64, f64)> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    // SAFETY: sysconf only reads a configuration value.
    let (page, tick) = unsafe { (libc::sysconf(libc::_SC_PAGESIZE), libc::sysconf(libc::_SC_CLK_TCK)) };
    parse_proc(&statm, &stat, page.max(1) as u64, tick.max(1) as f64)
}

/// The macOS (and other) reading through `procwatch`.
#[cfg(not(target_os = "linux"))]
pub fn read_self() -> Option<(u64, f64)> {
    procwatch::snapshot().map(|p| (p.resident, p.cpu_s))
}

/// Parses `/proc/self/statm` (resident pages are its second field) and `/proc/self/stat`
/// (utime and stime are fields 14 and 15; the name in field 2 may hold spaces, so count from
/// the last `)`).
pub fn parse_proc(statm: &str, stat: &str, page: u64, tick: f64) -> Option<(u64, f64)> {
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    let rest = &stat[stat.rfind(')')? + 1..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    let utime: f64 = f.get(11)?.parse().ok()?;
    let stime: f64 = f.get(12)?.parse().ok()?;
    Some((pages * page, (utime + stime) / tick))
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod tests;
