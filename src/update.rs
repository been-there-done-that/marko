//! `marko update`: fetch the latest GitHub release, verify its SHA-256, and replace this binary.
//!
//! The download is HTTPS-only and checked against the release's `SHA256SUMS` before anything is
//! installed. Linux and macOS replace the running binary atomically; Windows leaves a small
//! helper that finishes the move after this process exits, since a running `.exe` cannot be
//! replaced.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

const API: &str = "https://api.github.com/repos/been-there-done-that/marko/releases/latest";
const MAX_DOWNLOAD: u64 = 128 * 1024 * 1024;

pub fn run(check_only: bool) -> i32 {
    match try_run(check_only) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("marko: update failed: {e}");
            1
        }
    }
}

fn try_run(check_only: bool) -> Result<(), String> {
    let current = env!("CARGO_PKG_VERSION");
    let agent = agent();
    println!("Checking for updates…");
    let release = fetch_release(&agent)?;
    let latest = release.tag.trim_start_matches('v').to_string();
    if !is_newer(&latest, current) {
        println!("marko {current} is up to date.");
        return Ok(());
    }
    if check_only {
        println!("marko {latest} is available (current {current}).");
        return Ok(());
    }
    let asset = asset_name(std::env::consts::OS, std::env::consts::ARCH).ok_or_else(|| {
        format!("no release is built for {} {}.", std::env::consts::OS, std::env::consts::ARCH)
    })?;
    let archive = release.asset(asset).ok_or_else(|| format!("release {latest} has no {asset}."))?;
    let sums = release.asset("SHA256SUMS").ok_or_else(|| format!("release {latest} has no SHA256SUMS."))?;

    println!("Downloading marko {latest}…");
    let tmp = TempDir::new()?;
    let sums_text = download_string(&agent, &sums.url)?;
    let want = parse_sums(&sums_text, asset).ok_or_else(|| format!("SHA256SUMS has no entry for {asset}."))?;
    let archive_path = tmp.0.join(asset);
    download(&agent, &archive.url, &archive_path)?;
    let got = sha256_file(&archive_path)?;
    if got != want {
        return Err(format!("checksum mismatch for {asset}: got {got}, want {want}."));
    }
    println!("Installing…");
    install(&tmp.0, asset)?;
    println!("Updated marko {current} → {latest}. Restart Marko to use it.");
    Ok(())
}

struct Release {
    tag: String,
    assets: Vec<Asset>,
}

struct Asset {
    name: String,
    url: String,
}

impl Release {
    fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|a| a.name == name)
    }
}

fn agent() -> ureq::Agent {
    ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(180))
        .user_agent(concat!("marko/", env!("CARGO_PKG_VERSION")))
        .build()
}

fn fetch_release(agent: &ureq::Agent) -> Result<Release, String> {
    let text = agent
        .get(API)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("couldn't check GitHub: {e}"))?
        .into_string()
        .map_err(|e| format!("couldn't read the release: {e}"))?;
    let value: serde_json::Value = serde_json::from_str(&text).map_err(|e| format!("bad release JSON: {e}"))?;
    let tag = value["tag_name"].as_str().ok_or("the release has no tag.")?.to_string();
    let assets = value["assets"]
        .as_array()
        .ok_or("the release has no assets.")?
        .iter()
        .filter_map(|a| {
            Some(Asset {
                name: a["name"].as_str()?.to_string(),
                url: a["browser_download_url"].as_str()?.to_string(),
            })
        })
        .collect();
    Ok(Release { tag, assets })
}

fn download(agent: &ureq::Agent, url: &str, dest: &Path) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err(format!("refusing a non-HTTPS download: {url}"));
    }
    let response = agent.get(url).call().map_err(|e| format!("couldn't download {url}: {e}"))?;
    let mut reader = response.into_reader().take(MAX_DOWNLOAD + 1);
    let mut file = fs::File::create(dest).map_err(|e| format!("couldn't create {}: {e}", dest.display()))?;
    let n = std::io::copy(&mut reader, &mut file).map_err(|e| format!("couldn't write {}: {e}", dest.display()))?;
    if n > MAX_DOWNLOAD {
        return Err("the download is larger than 128 MB.".into());
    }
    file.sync_all().map_err(|e| format!("couldn't flush {}: {e}", dest.display()))?;
    Ok(())
}

fn download_string(agent: &ureq::Agent, url: &str) -> Result<String, String> {
    if !url.starts_with("https://") {
        return Err(format!("refusing a non-HTTPS download: {url}"));
    }
    let mut text = String::new();
    agent
        .get(url)
        .call()
        .map_err(|e| format!("couldn't download {url}: {e}"))?
        .into_reader()
        .take(1024 * 1024)
        .read_to_string(&mut text)
        .map_err(|e| format!("couldn't read {url}: {e}"))?;
    Ok(text)
}

/// The hash `SHA256SUMS` gives for `asset`, lowercased.
fn parse_sums(text: &str, asset: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let mut parts = line.split_whitespace();
        let hash = parts.next()?;
        let name = parts.next()?.trim_start_matches('*');
        (name == asset).then(|| hash.to_ascii_lowercase())
    })
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| format!("couldn't open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| format!("couldn't read {}: {e}", path.display()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
    }
    out
}

/// Whether `latest` is newer than `current`, comparing numeric parts and ignoring a leading `v`.
fn is_newer(latest: &str, current: &str) -> bool {
    let latest = version_parts(latest);
    let current = version_parts(current);
    for i in 0..latest.len().max(current.len()) {
        let a = *latest.get(i).unwrap_or(&0);
        let b = *current.get(i).unwrap_or(&0);
        if a != b {
            return a > b;
        }
    }
    false
}

fn version_parts(version: &str) -> Vec<u64> {
    version
        .trim_start_matches('v')
        .split(|c: char| !c.is_ascii_digit())
        .filter(|s| !s.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}

/// The release asset for a platform, named as `.github/workflows/release.yml` builds it.
fn asset_name(os: &str, arch: &str) -> Option<&'static str> {
    match (os, arch) {
        ("linux", "x86_64") => Some("marko-linux-x86_64.tar.gz"),
        ("linux", "aarch64") => Some("marko-linux-aarch64.tar.gz"),
        ("macos", "aarch64") => Some("marko-macos-aarch64.tar.gz"),
        ("macos", "x86_64") => Some("marko-macos-x86_64.tar.gz"),
        ("windows", "x86_64") => Some("marko-windows-x86_64.zip"),
        _ => None,
    }
}

struct TempDir(PathBuf);

impl TempDir {
    fn new() -> Result<Self, String> {
        let path = std::env::temp_dir().join(format!("marko-update-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).map_err(|e| format!("couldn't create {}: {e}", path.display()))?;
        Ok(Self(path))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
fn install(tmp: &Path, asset: &str) -> Result<(), String> {
    use std::path::Component;

    let extract = tmp.join("extract");
    fs::create_dir_all(&extract).map_err(|e| format!("couldn't create {}: {e}", extract.display()))?;
    let file = fs::File::open(tmp.join(asset)).map_err(|e| format!("couldn't open the archive: {e}"))?;
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    for entry in archive.entries().map_err(|e| format!("couldn't read the archive: {e}"))? {
        let mut entry = entry.map_err(|e| format!("couldn't read an archive entry: {e}"))?;
        let path = entry.path().map_err(|e| format!("bad archive path: {e}"))?.into_owned();
        if path.is_absolute() || path.components().any(|c| matches!(c, Component::ParentDir)) {
            return Err("the archive contains an unsafe path.".into());
        }
        if !entry.unpack_in(&extract).map_err(|e| format!("couldn't unpack the archive: {e}"))? {
            return Err("the archive contains a path outside the temporary folder.".into());
        }
    }
    let source = if cfg!(target_os = "macos") {
        extract.join("Marko.app/Contents/MacOS/marko")
    } else {
        extract.join("marko/marko")
    };
    if !source.is_file() {
        return Err(format!("the archive has no {}.", source.display()));
    }
    let exe = std::env::current_exe().map_err(|e| format!("couldn't find this binary: {e}"))?;
    let exe = fs::canonicalize(&exe).unwrap_or(exe);
    let new = exe.with_file_name(format!(".{}.new", exe.file_name().and_then(|n| n.to_str()).unwrap_or("marko")));
    fs::copy(&source, &new).map_err(|e| format!("couldn't write {}: {e}", new.display()))?;
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&new, fs::Permissions::from_mode(0o755))
            .map_err(|e| format!("couldn't make {} executable: {e}", new.display()))?;
    }
    fs::rename(&new, &exe).map_err(|e| {
        format!("couldn't replace {}: {e} (try the installer, or run with the permissions of that folder)", exe.display())
    })?;
    #[cfg(target_os = "macos")]
    if let Some(app) = exe
        .ancestors()
        .find(|p| p.extension().is_some_and(|e| e == std::ffi::OsStr::new("app")))
    {
        // The release is ad-hoc signed; re-sign the bundle after replacing its binary.
        let _ = std::process::Command::new("codesign").args(["--force", "--deep", "--sign", "-"]).arg(app).status();
    }
    Ok(())
}

#[cfg(windows)]
fn install(tmp: &Path, asset: &str) -> Result<(), String> {
    let file = fs::File::open(tmp.join(asset)).map_err(|e| format!("couldn't open the archive: {e}"))?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| format!("couldn't read the zip: {e}"))?;
    let mut entry = archive.by_name("marko.exe").map_err(|_| "the zip has no marko.exe.".to_string())?;
    let exe = std::env::current_exe().map_err(|e| format!("couldn't find this binary: {e}"))?;
    let new = exe.with_file_name("marko.new.exe");
    let mut out = fs::File::create(&new).map_err(|e| format!("couldn't write {}: {e}", new.display()))?;
    std::io::copy(&mut entry, &mut out).map_err(|e| format!("couldn't extract marko.exe: {e}"))?;
    out.sync_all().map_err(|e| format!("couldn't flush {}: {e}", new.display()))?;
    drop(out);
    let bat = std::env::temp_dir().join("marko-update.bat");
    let script = format!(
        "@echo off\r\nping 127.0.0.1 -n 3 >nul\r\nmove /y \"{}\" \"{}\"\r\nstart \"\" \"{}\"\r\ndel \"%~f0\"\r\n",
        new.display(),
        exe.display(),
        exe.display()
    );
    fs::write(&bat, script).map_err(|e| format!("couldn't write the update helper: {e}"))?;
    use std::os::windows::process::CommandExt;
    std::process::Command::new("cmd")
        .arg("/C")
        .arg(&bat)
        .creation_flags(0x0800_0000) // CREATE_NO_WINDOW
        .spawn()
        .map_err(|e| format!("couldn't start the update helper: {e}"))?;
    println!("The update finishes after Marko closes.");
    Ok(())
}

#[cfg(test)]
#[path = "update_tests.rs"]
mod tests;
