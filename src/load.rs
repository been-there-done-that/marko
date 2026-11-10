//! Reading a file into a document: size, binary and UTF-8 checks, frontmatter.
//! Task 2 fills in `load`, `strip_frontmatter` and `LoadError::message`; the types are fixed.

use std::path::Path;
use std::sync::Arc;

pub enum DocState {
    Loading,
    Ready { src: Arc<str>, large: bool, lossy: bool },
    Error(LoadError),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError { NotFound, Permission, Binary, TooLarge { bytes: u64 }, Io(String) }

const MB: u64 = 1024 * 1024;
const MAX_BYTES: u64 = 50 * MB;
const LARGE_BYTES: u64 = 5 * MB;
const BINARY_PROBE: usize = 8192;

/// Blocking; called only from workers.
pub fn load(path: &Path) -> DocState {
    // Check the size before reading, so an oversized file is never pulled into memory.
    let len = match std::fs::metadata(path) {
        Ok(m) => m.len(),
        Err(e) => return DocState::Error(map_io(e)),
    };
    if len > MAX_BYTES {
        return DocState::Error(LoadError::TooLarge { bytes: len });
    }
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(e) => return DocState::Error(map_io(e)),
    };
    if bytes[..bytes.len().min(BINARY_PROBE)].contains(&0) {
        return DocState::Error(LoadError::Binary);
    }
    let lossy = std::str::from_utf8(&bytes).is_err();
    let text = String::from_utf8_lossy(&bytes);
    DocState::Ready { src: Arc::from(text.as_ref()), large: len > LARGE_BYTES, lossy }
}

fn map_io(e: std::io::Error) -> LoadError {
    match e.kind() {
        std::io::ErrorKind::NotFound => LoadError::NotFound,
        std::io::ErrorKind::PermissionDenied => LoadError::Permission,
        _ => LoadError::Io(e.to_string()),
    }
}

/// Drops a leading YAML (`---`) or TOML (`+++`) frontmatter block.
/// The opener must be the first line and the block closes at a line equal to the same fence.
/// Without a closing fence the source is returned unchanged.
pub fn strip_frontmatter(src: &str) -> &str {
    let mut lines = src.split_inclusive('\n');
    let Some(first) = lines.next() else { return src };
    let fence = match first.trim_end_matches(['\r', '\n']) {
        "---" => "---",
        "+++" => "+++",
        _ => return src,
    };
    let mut end = first.len();
    for line in lines {
        end += line.len();
        if line.trim_end_matches(['\r', '\n']) == fence {
            return &src[end..];
        }
    }
    src
}

impl LoadError {
    pub fn message(&self) -> String {
        match self {
            LoadError::Binary => "This isn't a text file".into(),
            LoadError::TooLarge { .. } => "This file is too large to open (over 50 MB)".into(),
            LoadError::NotFound => "This file no longer exists".into(),
            LoadError::Permission => "You don't have permission to read this file".into(),
            LoadError::Io(e) => e.clone(),
        }
    }
}

#[cfg(test)]
#[path = "load_tests.rs"]
mod tests;
