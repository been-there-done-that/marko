//! Fetching a web image: the one place the app talks to the network. Callers decide whether a
//! fetch is allowed (the Web images setting, or the reader pressing Load); this only fetches.

use std::io::Read;
use std::time::Duration;

/// Longest a fetch may take, connecting and reading together.
pub const TIMEOUT: Duration = Duration::from_secs(15);
/// Largest body kept (20 MB).
pub const MAX_BYTES: u64 = 20 * 1024 * 1024;

/// Ok for an `http:` or `https:` address; anything else (`file:`, `data:`, a bare path) is refused.
pub fn check_url(url: &str) -> Result<(), String> {
    let lower = url.trim().to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        Ok(())
    } else {
        Err("only http and https addresses can be fetched".into())
    }
}

/// The bytes at `url`: 15 s at most, 20 MB at most. Blocking; call from the worker.
pub fn fetch(url: &str) -> Result<Vec<u8>, String> {
    fetch_with(url, MAX_BYTES, TIMEOUT)
}

fn fetch_with(url: &str, cap: u64, timeout: Duration) -> Result<Vec<u8>, String> {
    check_url(url)?;
    let agent = ureq::AgentBuilder::new().timeout(timeout).user_agent("marko").build();
    let resp = agent.get(url.trim()).call().map_err(|e| e.to_string())?;
    let too_large = || format!("too large (over {} MB)", cap / (1024 * 1024));
    if resp.header("content-length").and_then(|v| v.parse::<u64>().ok()).is_some_and(|n| n > cap) {
        return Err(too_large());
    }
    let mut body = Vec::new();
    resp.into_reader().take(cap + 1).read_to_end(&mut body).map_err(|e| e.to_string())?;
    if body.len() as u64 > cap {
        return Err(too_large());
    }
    Ok(body)
}

#[cfg(test)]
#[path = "net_tests.rs"]
mod tests;
