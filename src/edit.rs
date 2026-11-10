//! Edit buffers: the text being edited, its saved state, atomic saves and untitled names.

use std::path::Path;

use rune::TextEdit;

use crate::watch::content_hash;

/// The text an Edit tab holds, with the hash of the last saved (or opened) content.
pub struct EditBuffer {
    pub text: TextEdit,
    pub saved_hash: u64,
}

impl EditBuffer {
    pub fn from_src(src: &str) -> Self {
        Self { text: TextEdit::new(src), saved_hash: content_hash(src) }
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
    }
}

/// Writes `text` verbatim to a temp file beside `path`, then renames it over `path`, so a
/// reader never sees a half-written file.
pub fn save_atomic(path: &Path, text: &str) -> Result<(), String> {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
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
