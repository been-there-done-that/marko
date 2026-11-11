#![allow(clippy::field_reassign_with_default)]

use super::*;

#[test]
fn write_then_read_round_trips() {
    let dir = tempfile::tempdir().unwrap();
    assert!(read(dir.path()).is_none());
    let mut s = Settings::default();
    s.theme = "nordic".into();
    s.text_size = 19.0;
    write(dir.path(), &s).unwrap();
    assert_eq!(read(dir.path()), Some(s));
    // No temporary file is left behind.
    let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().flatten().map(|e| e.file_name()).collect();
    assert_eq!(names, vec![std::ffi::OsString::from(FILE)]);
}

#[test]
fn read_clamps_a_hand_edited_file() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(FILE), r#"{"text_size": 900}"#).unwrap();
    assert_eq!(read(dir.path()).unwrap().text_size, crate::settings::TEXT_SIZE_MAX);
}

#[test]
fn external_change_applies_and_echo_does_not() {
    let mut s = AppState::new();
    start_with(&s.settings);
    let mut theirs = s.settings.clone();
    theirs.theme = "paper".into();
    assert!(apply_external(&mut s, theirs.clone()));
    assert_eq!(s.settings.theme, "paper");
    // The same file seen again (an echo) changes nothing.
    assert!(!apply_external(&mut s, theirs));
}

#[test]
fn an_echo_of_an_older_own_write_does_not_rewind_a_drag() {
    // A slider drag: the writer saves 0.6 during a pause, the drag moves on to 0.7, and only then
    // does the watcher read the file back.
    let mut s = AppState::new();
    start_with(&s.settings);
    s.settings.background.panel_opacity = 0.6;
    let written = s.settings.clone();
    note_written(&written);
    remember_own(&s.settings);
    s.settings.background.panel_opacity = 0.7;
    remember_own(&s.settings);
    assert!(!apply_external(&mut s, written), "the file holds this window's own older value");
    assert_eq!(s.settings.background.panel_opacity, 0.7, "the drag keeps its newest value");
}

#[test]
fn another_windows_change_still_applies_after_own_writes() {
    let mut s = AppState::new();
    start_with(&s.settings);
    s.settings.text_size = 18.0;
    note_written(&s.settings);
    remember_own(&s.settings);
    let mut theirs = s.settings.clone();
    theirs.theme = "nordic".into();
    assert!(apply_external(&mut s, theirs));
    assert_eq!(s.settings.theme, "nordic");
}
