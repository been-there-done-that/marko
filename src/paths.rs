//! Path helpers: which files count as Markdown, canonical identity, tab titles.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

const EXTS: [&str; 5] = ["md", "markdown", "mdown", "mkd", "txt"];

/// True when the file has a Markdown extension (any case) and a non-empty stem.
pub fn is_markdown(p: &Path) -> bool {
    // `Path::new(".md")` has stem ".md" and no extension, so it fails the extension check.
    let has_stem = p.file_stem().is_some_and(|s| !s.is_empty());
    has_stem && p.extension().and_then(|e| e.to_str()).is_some_and(|e| EXTS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

/// The file's identity: `canonicalize`, or an absolute path when that fails.
pub fn canonical(p: &Path) -> PathBuf {
    std::fs::canonicalize(p).unwrap_or_else(|_| {
        if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir().map(|d| d.join(p)).unwrap_or_else(|_| p.to_path_buf()) }
    })
}

/// File names, with `" — parent"` added when two or more share a name.
pub fn display_titles(paths: &[PathBuf]) -> Vec<String> {
    let name = |p: &PathBuf| p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.display().to_string());
    let mut counts: HashMap<String, usize> = HashMap::new();
    for p in paths { *counts.entry(name(p)).or_default() += 1; }
    paths.iter().map(|p| {
        let n = name(p);
        if counts[&n] > 1 {
            let parent = p.parent().and_then(|d| d.file_name()).map(|d| d.to_string_lossy().into_owned()).unwrap_or_default();
            format!("{n} — {parent}")
        } else { n }
    }).collect()
}

#[cfg(test)]
#[path = "paths_tests.rs"]
mod tests;
