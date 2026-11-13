//! Unsaved-work recovery: the buffers that would be lost if the window closed, snapshotted to
//! the config folder on close and restored on the next launch.
//!
//! Rune's close hook cannot cancel a close, so a "save before quitting?" prompt is not possible.
//! Instead, every dirty buffer is written to `recovery.json` as the window goes, and the next
//! launch reopens those tabs with their edits still dirty. Saving or discarding a tab removes it
//! from the next snapshot, so a recovery never resurrects work the reader let go of.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::edit::EditBuffer;
use crate::load::{self, DocState, LARGE_BYTES};
use crate::paths::canonical;
use crate::watch::content_hash;

/// The recovery file's name in the config folder.
pub const FILE: &str = "recovery.json";
/// Largest recovery file read (256 MB): a bound on a hand-edited or corrupt file.
const MAX_RECOVERY_BYTES: u64 = 256 * 1024 * 1024;

/// One unsaved buffer: where it came from (None for an untitled tab), its name, and its text.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Recovered {
    pub path: Option<PathBuf>,
    pub name: String,
    pub text: String,
}

/// The dirty buffers in `s`, in tab order.
pub fn snapshot(s: &AppState) -> Vec<Recovered> {
    s.tabs
        .iter()
        .filter_map(|t| {
            let buf = t.edit.as_ref()?;
            if !buf.dirty() {
                return None;
            }
            Some(Recovered {
                path: (!t.untitled).then(|| t.path.clone()),
                name: t.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "Untitled.md".into()),
                text: buf.text(),
            })
        })
        .collect()
}

/// Writes `items` to `dir`, or removes the file when there is nothing to recover. Atomic:
/// a temp file in the same folder, then a rename.
pub fn write(dir: &Path, items: &[Recovered]) -> std::io::Result<()> {
    let path = dir.join(FILE);
    if items.is_empty() {
        return match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        };
    }
    std::fs::create_dir_all(dir)?;
    let json = serde_json::to_vec(items).map_err(std::io::Error::other)?;
    let tmp = dir.join(format!(".{FILE}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, json)?;
    // The snapshot holds document text: keep it private on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp, &path)
}

/// The recovery file's contents, or empty when there is none or it doesn't parse. A parsed file
/// is removed, so one launch restores it once.
pub fn read(dir: &Path) -> Vec<Recovered> {
    let path = dir.join(FILE);
    if std::fs::metadata(&path).map(|m| m.len() > MAX_RECOVERY_BYTES).unwrap_or(false) {
        return Vec::new();
    }
    let Ok(text) = std::fs::read_to_string(&path) else { return Vec::new() };
    match serde_json::from_str::<Vec<Recovered>>(&text) {
        Ok(items) => {
            let _ = std::fs::remove_file(&path);
            items
        }
        Err(_) => Vec::new(),
    }
}

/// Reopens each recovered buffer as a dirty tab. A file that still exists keeps its disk text as
/// the saved baseline, so the recovered edits are dirty against it and the conflict banner works.
pub fn restore(s: &mut AppState, items: Vec<Recovered>) {
    for item in items {
        if item.text.is_empty() && item.path.is_none() {
            continue;
        }
        let (id, disk) = match &item.path {
            Some(p) => {
                let key = canonical(p);
                let disk = match load::load(&key) {
                    DocState::Ready { src, .. } => src.to_string(),
                    _ => String::new(),
                };
                (s.tabs.open(&key).0, disk)
            }
            None => (s.tabs.open_untitled(&item.name), String::new()),
        };
        let Some(tab) = s.tabs.get_mut(id) else { continue };
        let large = item.text.len() as u64 > LARGE_BYTES;
        tab.doc = DocState::Ready { src: Arc::from(item.text.as_str()), large, lossy: false };
        tab.hash = Some(content_hash(&disk));
        tab.edit = Some(EditBuffer::recovered(&item.text, &disk));
        tab.untitled = item.path.is_none();
        tab.deleted = item.path.as_ref().is_some_and(|p| !p.is_file());
        tab.conflict = false;
        tab.changed = false;
        tab.rev = tab.rev.wrapping_add(1);
        tab.preview_rev = tab.rev;
    }
}

#[cfg(test)]
#[path = "recovery_tests.rs"]
mod tests;
