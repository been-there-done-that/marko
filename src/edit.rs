//! Edit buffers: the text being edited, its saved state, atomic saves and untitled names.

use std::path::Path;

use rune::TextEdit;

use crate::watch::content_hash;

/// The text an Edit tab holds, with the hash of the last saved (or opened) content.
pub struct EditBuffer {
    pub text: TextEdit,
    pub saved_hash: u64,
    /// Cheap O(1) hint for the tab strip. `dirty` (which hashes) stays authoritative for saves.
    edited: bool,
}

impl EditBuffer {
    pub fn from_src(src: &str) -> Self {
        Self { text: TextEdit::new(src), saved_hash: content_hash(src), edited: false }
    }

    /// A buffer restored from a recovery snapshot: `text` is what was being edited and `saved`
    /// is what is on disk now, so the buffer is dirty exactly when the two differ.
    pub fn recovered(text: &str, saved: &str) -> Self {
        Self { text: TextEdit::new(text), saved_hash: content_hash(saved), edited: text != saved }
    }

    /// Records that the text was edited through the input. The hash check stays in `dirty`.
    pub fn mark_edited(&mut self) {
        self.edited = true;
    }

    /// The O(1) dirty hint, for the tab strip's per-frame dot.
    pub fn dirty_hint(&self) -> bool {
        self.edited
    }

    pub fn text(&self) -> String {
        self.text.text.clone()
    }

    pub fn dirty(&self) -> bool {
        content_hash(&self.text.text) != self.saved_hash
    }

    /// Marks `saved` as what is on disk now. Text typed after the save began stays dirty.
    pub fn mark_saved(&mut self, saved: &str) {
        self.saved_hash = content_hash(saved);
        self.edited = self.text.text != saved;
    }
}

/// Writes `text` verbatim to a unique temp file beside `path`, syncs it, then renames it over
/// `path`, so a reader never sees a half-written file and a crash leaves the old one intact.
///
/// The temp name is unpredictable and created exclusively (`create_new`), so a planted symlink
/// or a second writer cannot redirect the write, and the existing file's mode is carried over.
pub fn save_atomic(path: &Path, text: &str) -> Result<(), String> {
    use std::io::Write as _;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "marko".into());
    // A file that exists keeps its mode; a new one takes the umask default at rename time.
    let mode = std::fs::metadata(path).ok().map(|m| m.permissions());

    let mut last_err = String::new();
    for _ in 0..8 {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let tmp = dir.join(format!(".{name}.{}.{n}.tmp", std::process::id()));
        let mut file = match std::fs::OpenOptions::new().write(true).create_new(true).open(&tmp) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                last_err = e.to_string();
                continue;
            }
            Err(e) => return Err(e.to_string()),
        };
        let written = (|| {
            file.write_all(text.as_bytes())?;
            file.sync_all()?;
            if let Some(perm) = &mode {
                std::fs::set_permissions(&tmp, perm.clone())?;
            }
            Ok::<(), std::io::Error>(())
        })();
        drop(file);
        if let Err(e) = written {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        if let Err(e) = std::fs::rename(&tmp, path) {
            let _ = std::fs::remove_file(&tmp);
            return Err(e.to_string());
        }
        // Make the rename itself durable: on Unix a directory fsync is what survives a crash.
        #[cfg(unix)]
        if let Ok(d) = std::fs::File::open(dir) {
            let _ = d.sync_all();
        }
        return Ok(());
    }
    Err(if last_err.is_empty() { "couldn't create a temporary file".into() } else { last_err })
}

/// The first free name of `Untitled.md`, `Untitled 2.md`, `Untitled 3.md`, …
pub fn untitled_name(existing: &[String]) -> String {
    let taken = |n: &str| existing.iter().any(|e| e == n);
    if !taken("Untitled.md") {
        return "Untitled.md".to_string();
    }
    let mut i = 2;
    loop {
        let name = format!("Untitled {i}.md");
        if !taken(&name) {
            return name;
        }
        i += 1;
    }
}

#[cfg(test)]
#[path = "edit_tests.rs"]
mod tests;
