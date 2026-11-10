//! This process's memory and CPU, sampled once a second for the titlebar readout. The numbers
//! come from `procwatch`, which reads the process's own counters.

use std::time::Duration;

use rune::Tx;

use crate::app::AppState;

/// How often the readout samples, in seconds.
pub const EVERY: f64 = 1.0;

/// What the readout shows: resident memory in whole MB, and CPU use since the last sample in
/// whole percent of one core.
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
    let rss_mb = (rss_bytes as f64 / (1u64 << 20) as f64).round() as u32;
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
    // `snapshot` is the reading that carries CPU time at this procwatch rev; it walks the
    // threads, which once a second is cheap.
    let reading = procwatch::snapshot().map(|p| (p.resident, p.cpu_s));
    let changed = apply(s, now, reading);
    if let Some(prev) = s.stats_last {
        tx.wake_after(Duration::from_secs_f64((prev.at + EVERY - now).max(0.0)));
    }
    changed
}

#[cfg(test)]
#[path = "stats_tests.rs"]
mod tests;
