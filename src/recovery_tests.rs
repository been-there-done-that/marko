use std::path::Path;
use std::sync::Arc;

use super::*;
use crate::app::AppState;
use crate::edit::EditBuffer;
use crate::load::DocState;

fn tab_with_edit(s: &mut AppState, path: &Path, disk: &str, edited: &str) -> u64 {
    let (id, _) = s.tabs.open(path);
    let tab = s.tabs.get_mut(id).unwrap();
    tab.doc = DocState::Ready { src: Arc::from(edited), large: false, lossy: false };
    tab.edit = Some(EditBuffer::recovered(edited, disk));
    id
}

#[test]
fn snapshot_keeps_only_dirty_tabs() {
    let dir = tempfile::tempdir().unwrap();
    let clean = dir.path().join("clean.md");
    let dirty = dir.path().join("dirty.md");
    std::fs::write(&clean, "same").unwrap();
    std::fs::write(&dirty, "old").unwrap();

    let mut s = AppState::new();
    let (id, _) = s.tabs.open(&clean);
    s.tabs.get_mut(id).unwrap().edit = Some(EditBuffer::from_src("same"));
    tab_with_edit(&mut s, &dirty, "old", "edited");

    let items = snapshot(&s);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].text, "edited");
    assert_eq!(items[0].path.as_deref(), Some(dirty.as_path()));
}

#[test]
fn untitled_tabs_snapshot_without_a_path() {
    let mut s = AppState::new();
    let id = s.tabs.open_untitled("Notes.md");
    let tab = s.tabs.get_mut(id).unwrap();
    tab.edit = Some(EditBuffer::recovered("draft", ""));

    let items = snapshot(&s);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].path, None);
    assert_eq!(items[0].name, "Notes.md");
    assert_eq!(items[0].text, "draft");
}

#[test]
fn write_then_read_round_trips_and_consumes() {
    let dir = tempfile::tempdir().unwrap();
    let items = vec![Recovered { path: Some(dir.path().join("a.md")), name: "a.md".into(), text: "edited".into() }];
    write(dir.path(), &items).unwrap();
    assert!(dir.path().join(FILE).is_file());

    assert_eq!(read(dir.path()), items);
    assert!(!dir.path().join(FILE).is_file(), "a restored snapshot is removed");
    assert!(read(dir.path()).is_empty());
}

#[test]
fn empty_snapshot_removes_the_file() {
    let dir = tempfile::tempdir().unwrap();
    let items = vec![Recovered { path: None, name: "x.md".into(), text: "x".into() }];
    write(dir.path(), &items).unwrap();
    write(dir.path(), &[]).unwrap();
    assert!(!dir.path().join(FILE).exists());
}

#[test]
fn restore_keeps_edits_dirty_against_the_disk_text() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.md");
    std::fs::write(&path, "on disk").unwrap();

    let mut s = AppState::new();
    restore(&mut s, vec![Recovered { path: Some(path.clone()), name: "a.md".into(), text: "unsaved edit".into() }]);

    let tab = s.tabs.iter().find(|t| t.path == canonical(&path)).expect("restored tab");
    assert!(tab.edit.as_ref().is_some_and(EditBuffer::dirty), "the edit survives as unsaved work");
    match &tab.doc {
        DocState::Ready { src, .. } => assert_eq!(&**src, "unsaved edit"),
        _ => panic!("recovered text is shown"),
    }
    assert_eq!(tab.hash, Some(content_hash("on disk")), "the disk text stays the save baseline");
    assert!(!tab.deleted);
}

#[test]
fn restore_marks_a_missing_file_deleted() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gone.md");

    let mut s = AppState::new();
    restore(&mut s, vec![Recovered { path: Some(path.clone()), name: "gone.md".into(), text: "still here".into() }]);

    let tab = s.tabs.iter().find(|t| t.path == canonical(&path)).expect("restored tab");
    assert!(tab.deleted);
    assert!(tab.edit.as_ref().is_some_and(EditBuffer::dirty));
}

#[test]
fn restore_dedupes_a_file_the_session_already_opened() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.md");
    std::fs::write(&path, "disk").unwrap();

    let mut s = AppState::new();
    s.tabs.open(&path);
    restore(&mut s, vec![Recovered { path: Some(path.clone()), name: "a.md".into(), text: "recovered".into() }]);

    assert_eq!(s.tabs.iter().count(), 1, "one tab, not a duplicate");
    let tab = s.tabs.active().unwrap();
    assert!(tab.edit.as_ref().is_some_and(EditBuffer::dirty));
}
