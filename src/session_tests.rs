use std::path::{Path, PathBuf};

use rune::prefs::Prefs;

use super::*;
use crate::app::{AppState, Appearance};
use crate::paths::canonical;

fn write(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, "# t").unwrap();
    p
}

#[test]
fn capture_then_restore_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().join("docs");
    std::fs::create_dir(&folder).unwrap();
    let a = write(dir.path(), "a.md");
    let b = write(&folder, "b.md");

    let mut s = AppState::new();
    s.open_paths(std::slice::from_ref(&folder), 0.0);
    s.open_paths(&[a.clone(), b.clone()], 0.0);
    let first = s.tabs.iter().next().unwrap().id;
    s.tabs.activate(first);
    s.zoom = 1.25;
    s.wide = true;
    s.appearance = Appearance::Dark;
    s.sidebar.visible = false;

    let prefs_path = dir.path().join("state").join("prefs.json");
    save(&mut Prefs::at(&prefs_path), &capture(&s));

    let mut r = AppState::new();
    restore(&mut r, load(&Prefs::at(&prefs_path)), 0.0);

    let tabs: Vec<PathBuf> = r.tabs.iter().map(|t| t.path.clone()).collect();
    assert_eq!(tabs, vec![canonical(&a), canonical(&b)]);
    assert_eq!(r.tabs.active().map(|t| t.path.clone()), Some(canonical(&a)));
    assert_eq!(r.sidebar.roots.len(), 1);
    assert_eq!(r.sidebar.roots[0].path, canonical(&folder));
    assert_eq!(r.zoom, 1.25);
    assert!(r.wide);
    assert_eq!(r.appearance, Appearance::Dark);
    assert!(!r.sidebar.visible);
    assert_eq!(r.recent, s.recent);
}

#[test]
fn restore_skips_missing_paths() {
    let dir = tempfile::tempdir().unwrap();
    let gone_file = dir.path().join("gone.md");
    let gone_dir = dir.path().join("gone");
    let session = Session {
        tabs: vec![gone_file.clone()],
        active: Some(0),
        roots: vec![gone_dir.clone()],
        recent: vec![gone_file.clone(), gone_dir.clone()],
        ..Default::default()
    };
    let mut s = AppState::new();
    restore(&mut s, session, 0.0);
    assert!(s.tabs.is_empty());
    assert!(s.sidebar.roots.is_empty());
    assert!(s.jobs.is_empty());
    assert!(!s.recent.contains(&gone_file));
    assert!(!s.recent.contains(&gone_dir));
    assert!(s.recent.is_empty());
}

#[test]
fn corrupt_session_falls_back_to_default() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("prefs.json");
    std::fs::write(&path, br#"{"session": 42}"#).unwrap();
    let prefs = Prefs::at(&path);
    let session = load(&prefs);
    assert_eq!(session, Session::default());

    let mut s = AppState::new();
    restore(&mut s, session, 0.0);
    assert!(s.tabs.is_empty());
    assert!(s.sidebar.visible);
    assert_eq!(s.zoom, 1.0);
    assert_eq!(s.appearance, Appearance::System);
}

#[test]
fn second_lock_in_same_dir_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let first = primary_lock(dir.path());
    assert!(first.is_some());
    assert!(primary_lock(dir.path()).is_none());
    drop(first);
    assert!(primary_lock(dir.path()).is_some());
}

#[test]
fn debounce_fires_once_after_one_second_quiet() {
    let mut d = SaveDebounce::default();
    assert!(!d.due(0.0), "nothing touched, nothing due");
    d.touch(0.0);
    assert!(!d.due(0.5));
    d.touch(0.8);
    assert!(!d.due(1.5), "only 0.7 s since the last change");
    assert!(d.due(1.9));
    assert!(!d.due(2.5), "fires once per burst");
}
