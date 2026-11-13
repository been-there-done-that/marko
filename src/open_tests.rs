use std::path::{Path, PathBuf};

use crate::app::{AppState, Job};
use crate::paths::canonical;
use crate::sidebar::scan;

fn write(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, "# t").unwrap();
    p
}

fn loads(s: &AppState) -> usize {
    s.jobs.iter().filter(|j| matches!(j, Job::Load { .. })).count()
}

/// Runs a scan the way the worker does: it builds the tree and picks the file to open.
fn scanned(s: &mut AppState, root: &Path) {
    match scan(root) {
        Ok(files) => {
            let open = crate::sidebar::readme_or_first(&files);
            s.scanned(root, Ok(crate::sidebar::build_tree(&files, root)), open);
        }
        Err(e) => s.scanned(root, Err(e), None),
    }
}

#[test]
fn mixed_drop_routes_each_kind() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("docs");
    std::fs::create_dir(&folder).unwrap();
    write(&folder, "x.md");
    let a = write(dir.path(), "a.md");
    let png = write(dir.path(), "b.png");
    let mut s = AppState::new();
    s.open_paths(&[folder, a, png], 1.0);
    assert_eq!(s.sidebar.roots.len(), 1);
    assert_eq!(s.tabs.iter().count(), 1);
    assert_eq!(s.toast.as_ref().map(|t| t.text.as_str()), Some("Skipped 1 file that isn't Markdown"));
    assert!(matches!(s.jobs.as_slice(), [Job::Load { .. }, Job::Scan { .. }]));
}

#[test]
fn opening_same_file_twice_queues_one_load() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(dir.path(), "a.md");
    let mut s = AppState::new();
    s.open_paths(std::slice::from_ref(&a), 0.0);
    s.open_paths(&[a], 0.0);
    assert_eq!(loads(&s), 1);
    assert_eq!(s.tabs.iter().count(), 1);
}

#[test]
fn recent_is_front_dedupe_capped_16() {
    let mut s = AppState::new();
    for i in 0..20 {
        s.remember_recent(Path::new(&format!("/nonexistent-rune/{i}.md")));
    }
    assert_eq!(s.recent.len(), 16);
    assert_eq!(s.recent[0], canonical(Path::new("/nonexistent-rune/19.md")));
    s.remember_recent(Path::new("/nonexistent-rune/10.md"));
    assert_eq!(s.recent.len(), 16);
    assert_eq!(s.recent[0], canonical(Path::new("/nonexistent-rune/10.md")));
    assert_eq!(s.recent.iter().filter(|p| **p == canonical(Path::new("/nonexistent-rune/10.md"))).count(), 1);
}

#[test]
fn scan_result_opens_readme_when_no_tabs() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.md");
    write(dir.path(), "README.md");
    let mut s = AppState::new();
    s.open_paths(&[dir.path().to_path_buf()], 0.0);
    let root = s.sidebar.roots[0].path.clone();
    scanned(&mut s, &root);
    assert_eq!(s.tabs.active().unwrap().path, root.join("README.md"));
    assert!(!s.sidebar.roots[0].scanning);
}

#[test]
fn scan_result_does_not_open_when_tabs_exist() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("docs");
    std::fs::create_dir(&folder).unwrap();
    write(&folder, "README.md");
    let a = write(dir.path(), "a.md");
    let mut s = AppState::new();
    s.open_paths(std::slice::from_ref(&a), 0.0);
    s.open_paths(std::slice::from_ref(&folder), 0.0);
    let root = s.sidebar.roots[0].path.clone();
    scanned(&mut s, &root);
    assert_eq!(s.tabs.iter().count(), 1);
    assert_eq!(s.tabs.active().unwrap().path, canonical(&a));
}

#[test]
fn scan_error_sets_root_error() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = AppState::new();
    s.open_paths(&[dir.path().to_path_buf()], 0.0);
    let root = s.sidebar.roots[0].path.clone();
    s.scanned(&root, Err("denied".to_string()), None);
    assert_eq!(s.sidebar.roots[0].error.as_deref(), Some("denied"));
    assert!(!s.sidebar.roots[0].scanning);
}

#[test]
fn toast_expires_after_2_4_seconds() {
    let mut s = AppState::new();
    s.show_toast("hi", 10.0);
    assert!(!s.tick(12.39));
    assert!(s.toast.is_some());
    assert!(s.tick(12.5));
    assert!(s.toast.is_none());
    assert!(!s.tick(13.0));
}
