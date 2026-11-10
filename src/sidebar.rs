//! Sidebar: the folder roots the app shows, their Markdown scan and the tree built from it.

use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use rune_kit::{FileNode, FileTree, TreeModel};

use crate::paths::{canonical, is_markdown};

/// Most Markdown files one root's scan keeps.
pub const SCAN_CAP: usize = 20_000;

/// Folders the scan never enters, besides hidden and git-ignored ones.
const SKIP_DIRS: [&str; 3] = ["node_modules", "target", ".venv"];

/// A folder the sidebar shows.
pub struct Root {
    pub path: PathBuf,
    pub name: String,
    pub tree: TreeModel<FileNode<PathBuf>>,
    pub scanning: bool,
    pub error: Option<String>,
}

/// The folders open in the sidebar, in the order they were added.
pub struct Sidebar {
    pub roots: Vec<Root>,
    pub visible: bool,
}

/// What [`Sidebar::add_root`] did with a folder.
pub enum AddRoot {
    Added(usize),
    // The root index is for reveal-on-select (phase 1b); nothing reads it yet.
    AlreadyRoot(#[allow(dead_code)] usize),
    InsideRoot {
        #[allow(dead_code)]
        root: usize,
    },
}

impl Sidebar {
    pub fn new() -> Self {
        Sidebar { roots: Vec::new(), visible: true }
    }

    /// Adds a folder as a root. Compares canonical paths; a folder inside an existing root is not added.
    pub fn add_root(&mut self, dir: &Path) -> AddRoot {
        let path = canonical(dir);
        if let Some(root) = self.roots.iter().position(|r| r.path == path) {
            return AddRoot::AlreadyRoot(root);
        }
        if let Some(root) = self.roots.iter().position(|r| path.starts_with(&r.path)) {
            return AddRoot::InsideRoot { root };
        }
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        self.roots.push(Root { path, name, tree: TreeModel::new(), scanning: true, error: None });
        AddRoot::Added(self.roots.len() - 1)
    }

    pub fn remove_root(&mut self, index: usize) {
        if index < self.roots.len() {
            self.roots.remove(index);
        }
    }

    /// The index of the root that contains this canonical path.
    pub fn root_of(&self, file: &Path) -> Option<usize> {
        self.roots.iter().position(|r| file.starts_with(&r.path))
    }
}

/// Markdown files under `dir`, relative to it and sorted. Blocking; call from a worker only.
pub fn scan(dir: &Path) -> Result<Vec<PathBuf>, String> {
    scan_with_cap(dir, SCAN_CAP)
}

fn scan_with_cap(dir: &Path, cap: usize) -> Result<Vec<PathBuf>, String> {
    std::fs::read_dir(dir).map_err(|e| e.to_string())?;
    let walk = WalkBuilder::new(dir)
        .hidden(true)
        .git_ignore(true)
        // Honour a `.gitignore` even when the folder is not inside a git repository.
        .require_git(false)
        .filter_entry(|e| e.depth() == 0 || !e.file_name().to_str().is_some_and(|n| SKIP_DIRS.contains(&n)))
        .build();
    let mut out = Vec::new();
    for entry in walk {
        if out.len() >= cap {
            break;
        }
        let Ok(entry) = entry else { continue };
        if !entry.file_type().is_some_and(|t| t.is_file()) || !is_markdown(entry.path()) {
            continue;
        }
        if let Ok(rel) = entry.path().strip_prefix(dir) {
            out.push(rel.to_path_buf());
        }
    }
    out.sort();
    Ok(out)
}

/// A file tree over `files` (relative to `base`), folders first.
pub fn build_tree(root_key: &str, files: &[PathBuf], base: &Path) -> TreeModel<FileNode<PathBuf>> {
    let items: Vec<(String, PathBuf)> = files
        .iter()
        .map(|rel| {
            let key = rel.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
            (key, base.join(rel))
        })
        .collect();
    FileTree::new().key(root_key).dirs_first(true).build(items)
}

/// The top-level `README.md` (any case) if there is one, else the first file.
pub fn readme_or_first(files: &[PathBuf]) -> Option<PathBuf> {
    let top_readme = files.iter().find(|f| {
        f.parent().is_some_and(|p| p.as_os_str().is_empty())
            && f.file_name().is_some_and(|n| n.to_string_lossy().eq_ignore_ascii_case("readme.md"))
    });
    top_readme.or(files.first()).cloned()
}

#[cfg(test)]
#[path = "sidebar_tests.rs"]
mod tests;
