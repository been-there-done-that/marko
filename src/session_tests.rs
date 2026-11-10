use std::path::{Path, PathBuf};

use rune::prefs::Prefs;

use super::*;
use crate::app::AppState;
use crate::settings::Startup;
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
    s.settings.theme = "nordic".into();
    s.settings.text_size = 18.0;
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
    assert_eq!(r.settings.theme, "nordic");
    assert_eq!(r.settings.text_size, 18.0);
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
    assert_eq!(s.settings.theme, crate::settings::AUTO_THEME);
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

#[test]
fn session_round_trips_scroll() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.md");
    let b = dir.path().join("b.md");
    std::fs::write(&a, "# a").unwrap();
    std::fs::write(&b, "# b").unwrap();

    let mut s = AppState::new();
    s.open_paths(&[a.clone(), b.clone()], 0.0);
    let ids: Vec<u64> = s.tabs.iter().map(|t| t.id).collect();
    for (id, offset) in ids.into_iter().zip([120.0, 480.5]) {
        if let Some(t) = s.tabs.get_mut(id) {
            t.scroll = offset;
        }
    }

    let prefs_path = dir.path().join("state").join("prefs.json");
    save(&mut Prefs::at(&prefs_path), &capture(&s));
    let loaded = load(&Prefs::at(&prefs_path));
    assert_eq!(loaded.scroll, vec![120.0, 480.5]);

    let mut r = AppState::new();
    restore(&mut r, loaded, 0.0);
    let offsets: Vec<f32> = r.tabs.iter().map(|t| t.scroll).collect();
    assert_eq!(offsets, vec![120.0, 480.5]);
}

#[test]
fn old_session_without_scroll_loads() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.md");
    std::fs::write(&a, "# a").unwrap();
    let mut s = AppState::new();
    s.open_paths(std::slice::from_ref(&a), 0.0);
    let mut json = serde_json::to_value(capture(&s)).unwrap();
    json.as_object_mut().unwrap().remove("scroll");
    let path = dir.path().join("prefs.json");
    std::fs::write(&path, serde_json::json!({ "session": json }).to_string()).unwrap();

    let session = load(&Prefs::at(&path));
    assert!(session.scroll.is_empty());
    assert_eq!(session.tabs, vec![canonical(&a)]);
    let mut r = AppState::new();
    restore(&mut r, session, 0.0);
    assert_eq!(r.tabs.iter().count(), 1);
    assert_eq!(r.tabs.active().map(|t| t.scroll), Some(0.0));
}

#[test]
fn old_session_keeps_its_appearance_as_the_theme() {
    let session: Session = serde_json::from_str(r#"{"tabs":[],"active":null,"roots":[],"sidebar_visible":true,"zoom":1.0,"wide":false,"appearance":"dark","recent":[],"frame":null}"#).unwrap();
    assert_eq!(settings_of(&session).theme, "dark");
}

#[test]
fn empty_startup_skips_tabs_but_keeps_settings() {
    let dir = tempfile::tempdir().unwrap();
    let a = write(dir.path(), "a.md");
    let mut s = AppState::new();
    s.open_paths(std::slice::from_ref(&a), 0.0);
    s.settings.startup = Startup::Empty;
    let mut r = AppState::new();
    restore(&mut r, capture(&s), 0.0);
    assert!(r.tabs.is_empty());
    assert_eq!(r.settings.startup, Startup::Empty);
}

#[test]
fn usable_frame_accepts_a_sane_window_and_rejects_the_rest() {
    assert_eq!(usable_frame((100, 80, 1200, 800)), Some(((100, 80), (1200, 800))));
    assert_eq!(usable_frame((-1920, -20, 1100, 760)), Some(((-1920, -20), (1100, 760))), "a left-hand monitor");
    assert_eq!(usable_frame((0, 0, 100, 100)), None, "a sliver");
    assert_eq!(usable_frame((0, 0, 560, 399)), None);
    assert_eq!(usable_frame((0, 0, 20_000, 800)), None);
    assert_eq!(usable_frame((90_000, 0, 1100, 760)), None, "far off any desktop");
    assert_eq!(usable_frame((0, -40_000, 1100, 760)), None);
}

#[test]
fn frame_of_skips_windows_that_are_not_ordinary() {
    let base = rune::WindowPlacement { position: (10, 20), content_size: (1100, 760), scale: 2.0, ..Default::default() };
    assert_eq!(frame_of(&base), Some((10, 20, 1100, 760)));
    assert_eq!(frame_of(&rune::WindowPlacement { maximized: true, ..base }), None);
    assert_eq!(frame_of(&rune::WindowPlacement { minimized: true, ..base }), None);
    assert_eq!(frame_of(&rune::WindowPlacement { full_screen: true, ..base }), None);
}

#[test]
fn a_session_without_a_frame_still_loads() {
    let json = r#"{"tabs":[],"active":null,"roots":[],"sidebar_visible":true,"zoom":1.0,"wide":false,"appearance":"system","recent":[],"frame":null}"#;
    let s: Session = serde_json::from_str(json).unwrap();
    assert_eq!(s.frame, None);
}

#[test]
fn migrate_prefs_copies_session_and_themes_once() {
    let dir = tempfile::tempdir().unwrap();
    let old_path = dir.path().join("old").join("prefs.json");
    let new_path = dir.path().join("new").join("prefs.json");
    let mut old = Prefs::at(&old_path);
    let mut saved = Session::default();
    saved.zoom = 1.5;
    save(&mut old, &saved);
    std::fs::create_dir_all(dir.path().join("old/themes")).unwrap();
    std::fs::write(dir.path().join("old/themes/mine.json"), "{}").unwrap();

    migrate_prefs(&old, &Prefs::at(&new_path));
    assert_eq!(load(&Prefs::at(&new_path)).zoom, 1.5);
    assert!(dir.path().join("new/themes/mine.json").is_file());

    // A second launch keeps what the new folder has.
    let mut changed = Session::default();
    changed.zoom = 0.8;
    save(&mut Prefs::at(&new_path), &changed);
    migrate_prefs(&old, &Prefs::at(&new_path));
    assert_eq!(load(&Prefs::at(&new_path)).zoom, 0.8);
}
