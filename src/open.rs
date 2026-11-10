//! Routing for opened paths (drops, the Open dialog, the command line): files become
//! tabs, folders become sidebar roots.

use std::path::{Path, PathBuf};

use rune::FileDialogResult;

use crate::app::{AppState, Job};
use crate::paths::is_markdown;
use crate::sidebar::AddRoot;

/// Opened paths sorted by kind. `rejected` counts the paths that are neither folders nor Markdown.
pub struct Classified {
    pub files: Vec<PathBuf>,
    pub folders: Vec<PathBuf>,
    pub rejected: usize,
}

pub fn classify(paths: &[PathBuf]) -> Classified {
    let mut out = Classified { files: Vec::new(), folders: Vec::new(), rejected: 0 };
    for p in paths {
        if p.is_dir() {
            out.folders.push(p.clone());
        } else if is_markdown(p) {
            out.files.push(p.clone());
        } else {
            out.rejected += 1;
        }
    }
    out
}

impl AppState {
    /// Routes opened paths. Files go first, then folders, so a mixed drop queues its
    /// loads before its scans. Non-Markdown paths get one toast for the whole call.
    pub fn open_paths(&mut self, paths: &[PathBuf], now: f64) {
        let c = classify(paths);
        for f in &c.files {
            self.open_file(f);
        }
        for d in &c.folders {
            if let AddRoot::Added(i) = self.sidebar.add_root(d) {
                let root = self.sidebar.roots[i].path.clone();
                self.jobs.push(Job::Scan { root: root.clone() });
                self.remember_recent(&root);
            }
        }
        if c.rejected > 0 {
            let text = if c.rejected == 1 {
                "Skipped 1 file that isn't Markdown".to_string()
            } else {
                format!("Skipped {} files that aren't Markdown", c.rejected)
            };
            self.show_toast(text, now);
        }
    }

    /// Routes the answer from the Open File or Open Folder dialog. A cancelled dialog does
    /// nothing; a dialog that failed to open says so in a toast.
    pub fn dialog_answer(&mut self, r: FileDialogResult, now: f64) {
        match r {
            Ok(Some(path)) => self.open_paths(&[path], now),
            Ok(None) => {}
            Err(_) => self.show_toast("Couldn't open the file dialog", now),
        }
    }

    /// Opens one Markdown file as a tab. Queues a load only when the tab is new.
    pub(crate) fn open_file(&mut self, path: &Path) {
        let (id, created) = self.tabs.open(path);
        if created && let Some(tab) = self.tabs.get_mut(id) {
            let tab_path = tab.path.clone();
            self.jobs.push(Job::Load { tab: id, path: tab_path });
        }
        self.remember_recent(path);
    }
}

#[cfg(test)]
#[path = "open_tests.rs"]
mod tests;
