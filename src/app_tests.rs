use std::sync::Arc;

use super::*;
use crate::edit::EditBuffer;
use crate::load::DocState;

fn ready_tab(s: &mut AppState, path: &Path, text: &str) -> u64 {
    let (id, _) = s.tabs.open(path);
    let tab = s.tabs.get_mut(id).unwrap();
    tab.doc = DocState::Ready { src: Arc::from(text), large: false, lossy: false };
    tab.edit = Some(EditBuffer::from_src(text));
    id
}

#[test]
fn failed_save_keeps_the_tab_open() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.md");
    std::fs::write(&path, "old").unwrap();
    let mut s = AppState::new();
    let id = ready_tab(&mut s, &path, "old");
    s.queue_save(id, path.clone(), true, false);
    assert!(s.close_after_save.contains(&id));

    s.saved(id, "old".into(), Err("disk full".into()));
    assert!(s.tabs.iter().any(|t| t.id == id), "a failed save must not close the tab");
}

#[test]
fn successful_save_closes_the_tab() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.md");
    std::fs::write(&path, "old").unwrap();
    let mut s = AppState::new();
    let id = ready_tab(&mut s, &path, "old");
    s.queue_save(id, path, true, false);
    s.saved(id, "new".into(), Ok(()));
    assert!(!s.tabs.iter().any(|t| t.id == id), "a successful save closes the guarded tab");
}

#[test]
fn lossy_documents_are_not_saved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.md");
    std::fs::write(&path, "old").unwrap();
    let mut s = AppState::new();
    let id = ready_tab(&mut s, &path, "old");
    s.tabs.get_mut(id).unwrap().doc = DocState::Ready { src: Arc::from("old"), large: false, lossy: true };

    s.queue_save(id, path, false, false);
    assert!(s.jobs.is_empty(), "a lossy document must not be queued for writing");
    let toast = s.toast.as_ref().expect("a toast explains why");
    assert!(toast.text.contains("characters"), "{}", toast.text);
}

#[test]
fn lossy_documents_can_be_saved_as_a_copy() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.md");
    std::fs::write(&path, "old").unwrap();
    let mut s = AppState::new();
    let id = ready_tab(&mut s, &path, "old");
    s.tabs.get_mut(id).unwrap().doc = DocState::Ready { src: Arc::from("old"), large: false, lossy: true };

    s.queue_save(id, dir.path().join("copy.md"), false, true);
    assert!(s.jobs.iter().any(|j| matches!(j, Job::Save { .. })), "Save As is allowed");
}

#[test]
fn reload_while_dirty_raises_the_conflict_banner() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.md");
    std::fs::write(&path, "disk").unwrap();
    let mut s = AppState::new();
    let id = ready_tab(&mut s, &path, "disk");
    s.tabs.get_mut(id).unwrap().edit = Some(EditBuffer::recovered("edited", "disk"));

    s.requeue_active_load();
    let tab = s.tabs.iter().find(|t| t.id == id).unwrap();
    assert!(tab.conflict, "unsaved edits raise the banner instead of reloading");
    assert!(matches!(&tab.doc, DocState::Ready { .. }), "the document is not put back into Loading");
    assert!(s.jobs.is_empty(), "no load is queued");
}

#[test]
fn reload_while_clean_requeues_the_load() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.md");
    std::fs::write(&path, "disk").unwrap();
    let mut s = AppState::new();
    let id = ready_tab(&mut s, &path, "disk");

    s.requeue_active_load();
    let tab = s.tabs.iter().find(|t| t.id == id).unwrap();
    assert!(!tab.conflict);
    assert!(matches!(&tab.doc, DocState::Loading));
    assert!(s.jobs.iter().any(|j| matches!(j, Job::Load { tab: t, .. } if *t == id)));
}

#[test]
fn a_changed_pending_image_is_retried() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pic.png");
    std::fs::write(&path, b"x").unwrap();
    let mut s = AppState::new();
    s.images.mark_pending(path.clone());

    s.refresh_image(&path);
    assert!(s.jobs.iter().any(|j| matches!(j, Job::Image { key } if *key == path)));
}
