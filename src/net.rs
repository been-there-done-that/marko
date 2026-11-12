//! Fetching a web image: the one place the app talks to the network. Callers decide whether a
//! fetch is allowed (the Web images setting, or the reader pressing Load); this only fetches.

use std::io::Read;
use std::net::IpAddr;
use std::time::Duration;

use url::Url;

/// Longest a fetch may take, per request.
pub const TIMEOUT: Duration = Duration::from_secs(15);
/// Largest body kept (20 MB).
pub const MAX_BYTES: u64 = 20 * 1024 * 1024;
/// Redirects followed, each re-checked against the host policy.
const MAX_REDIRECTS: usize = 5;

/// Ok for an `http:` or `https:` address whose host is not local, private or a metadata name.
pub fn check_url(url: &str) -> Result<(), String> {
    let parsed = Url::parse(url.trim()).map_err(|e| e.to_string())?;
    check_scheme(&parsed)?;
    let host = parsed.host_str().ok_or("the address has no host")?;
    if !host_is_public(host) {
        return Err("refusing to fetch a local or private address".into());
    }
    Ok(())
}

fn check_scheme(parsed: &Url) -> Result<(), String> {
    match parsed.scheme() {
        "http" | "https" => Ok(()),
        _ => Err("only http and https addresses can be fetched".into()),
    }
}

/// True unless `host` is loopback, private, link-local, unspecified or a known metadata name.
///
/// A DNS name is not resolved here. A name that resolves to a private address (DNS rebinding)
/// is a residual risk that only pinning the connection's IP could close; ureq does not expose
/// the resolved address. Blocking the literal ranges and the common metadata names still stops
/// the direct and redirect-based cases.
fn host_is_public(host: &str) -> bool {
    let h = host.trim_matches(['[', ']']).trim_end_matches('.').to_ascii_lowercase();
    if h == "localhost" || h.ends_with(".localhost") {
        return false;
    }
    const METADATA: [&str; 3] = ["metadata.google.internal", "metadata.goog", "instance-data"];
    if METADATA.contains(&h.as_str()) {
        return false;
    }
    match h.parse::<IpAddr>() {
        Ok(ip) => ip_is_public(ip),
        Err(_) => true,
    }
}

fn ip_is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || v4.is_documentation()
                || v4.is_multicast()
                // 100.64.0.0/10, carrier-grade NAT
                || (v4.octets()[0] == 100 && (64..=127).contains(&v4.octets()[1])))
        }
        IpAddr::V6(v6) => {
            !(v6.is_loopback()
                || v6.is_unspecified()
                || v6.is_multicast()
                // fc00::/7 unique-local, fe80::/10 link-local
                || (v6.segments()[0] & 0xfe00 == 0xfc00)
                || (v6.segments()[0] & 0xffc0 == 0xfe80)
                // ::ffff:a.b.c.d is the IPv4 address underneath
                || v6.to_ipv4_mapped().is_some_and(|v4| !ip_is_public(IpAddr::V4(v4))))
        }
    }
}

/// The bytes at `url`: 15 s per request, 20 MB at most. Blocking; call from the worker.
pub fn fetch(url: &str) -> Result<Vec<u8>, String> {
    fetch_with(url, MAX_BYTES, TIMEOUT)
}

fn fetch_with(url: &str, cap: u64, timeout: Duration) -> Result<Vec<u8>, String> {
    // Unit tests serve from 127.0.0.1, which the production policy refuses.
    fetch_with_policy(url, cap, timeout, cfg!(test))
}

fn fetch_with_policy(url: &str, cap: u64, timeout: Duration, allow_local: bool) -> Result<Vec<u8>, String> {
    // Redirects are followed by hand so each hop is checked, not just the first URL.
    let agent = ureq::AgentBuilder::new().timeout(timeout).redirects(0).user_agent("marko").build();
    let mut current = Url::parse(url.trim()).map_err(|e| e.to_string())?;
    let mut first = true;
    for _ in 0..=MAX_REDIRECTS {
        // The test server is local; only its first hop is exempt, so redirects are still checked.
        if allow_local && first {
            check_scheme(&current)?;
        } else {
            check_url(current.as_str())?;
        }
        first = false;
        let too_large = || format!("too large (over {} MB)", cap / (1024 * 1024));
        let resp = match agent.get(current.as_str()).call() {
            Ok(r) => r,
            Err(ureq::Error::Status(code, resp)) if (300..400).contains(&code) => resp,
            Err(e) => return Err(e.to_string()),
        };
        // With redirects(0) ureq may hand a 3xx back as Ok, so check the status either way.
        if (300..400).contains(&resp.status()) {
            let loc = resp.header("location").ok_or("a redirect without a location")?;
            current = current.join(loc).map_err(|e| e.to_string())?;
            continue;
        }
        if resp.header("content-length").and_then(|v| v.parse::<u64>().ok()).is_some_and(|n| n > cap) {
            return Err(too_large());
        }
        let mut body = Vec::new();
        resp.into_reader().take(cap + 1).read_to_end(&mut body).map_err(|e| e.to_string())?;
        if body.len() as u64 > cap {
            return Err(too_large());
        }
        return Ok(body);
    }
    Err("too many redirects".into())
}

#[cfg(test)]
#[path = "net_tests.rs"]
mod tests;
