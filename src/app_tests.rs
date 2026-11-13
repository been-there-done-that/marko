use std::sync::Arc;

use super::*;
use crate::edit::EditBuffer;
use crate::load::DocState;

fn ready_tab(s: &mut AppState, path: &Path, text: &str) -> u64 {
    let (id, _) = s.tabs.open(path);
    let tab = s.tabs.get_mut(id).unwrap();
    tab.doc = DocState::Ready { src: Arc::new(text.into()), large: false, lossy: false };
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
    s.tabs.get_mut(id).unwrap().doc = DocState::Ready { src: Arc::new("old".into()), large: false, lossy: true };

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
    s.tabs.get_mut(id).unwrap().doc = DocState::Ready { src: Arc::new("old".into()), large: false, lossy: true };

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
fn a_changed_failed_image_is_retried() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pic.png");
    std::fs::write(&path, b"x").unwrap();
    let mut s = AppState::new();
    s.images.mark_failed(path.clone());

    s.refresh_image(&path);
    assert!(s.jobs.iter().any(|j| matches!(j, Job::Image { key } if *key == path)));
    assert!(!s.images.failed(&path), "the retry clears the failure");
}

#[test]
fn image_window_follows_the_scroll() {
    let paths: Vec<PathBuf> = (0..100).map(|i| PathBuf::from(format!("/i{i}.png"))).collect();
    assert_eq!(window(&paths, 0.0).len(), LAZY_IMAGE_WINDOW, "the top window");
    let mid = window(&paths, 0.5);
    assert_eq!(mid.len(), LAZY_IMAGE_WINDOW * 3, "the window plus the top");
    assert!(mid.contains(&paths[48]) && mid.contains(&paths[51]));
    assert!(mid.contains(&paths[0]), "the top stays ready for a scroll back");
    assert_eq!(window(&paths[..4], 0.5).len(), 4, "a short list is decoded whole");
}

#[test]
fn a_background_tab_queues_no_images() {
    let mut s = AppState::new();
    let a = s.tabs.open(Path::new("/a.md")).0;
    let _b = s.tabs.open(Path::new("/b.md")).0;
    let path = PathBuf::from("/a.png");
    s.load_images(a, vec![path.clone()]);
    assert!(s.jobs.is_empty(), "an inactive tab waits until it is shown");
    s.tabs.activate(a);
    s.want_visible_images(a);
    assert!(s.jobs.iter().any(|j| matches!(j, Job::Image { key } if *key == path)));
}

#[test]
fn a_long_document_queues_only_the_window() {
    let mut s = AppState::new();
    let id = s.tabs.open(Path::new("/long.md")).0;
    let images: Vec<PathBuf> = (0..100).map(|i| PathBuf::from(format!("/img{i}.png"))).collect();
    if let Some(t) = s.tabs.get_mut(id) {
        t.images = images;
        t.scroll_at.set([500.0, 1000.0, 0.0, 0.0]);
    }
    s.want_visible_images(id);
    let queued: Vec<usize> = s
        .jobs
        .iter()
        .filter_map(|j| match j {
            Job::Image { key } => key.to_str()?.strip_prefix("/img")?.strip_suffix(".png")?.parse().ok(),
            _ => None,
        })
        .collect();
    assert!(queued.iter().all(|i| (48..52).contains(i) || *i < 2), "{queued:?}");
    assert_eq!(queued.len(), MAX_IN_FLIGHT_IMAGES);
}

#[test]
fn the_image_queue_refills_as_decodes_finish() {
    let mut s = AppState::new();
    let id = s.tabs.open(Path::new("/long.md")).0;
    let images: Vec<PathBuf> = (0..100).map(|i| PathBuf::from(format!("/img{i}.png"))).collect();
    if let Some(t) = s.tabs.get_mut(id) {
        t.images = images;
        t.scroll_at.set([500.0, 1000.0, 0.0, 0.0]);
    }
    s.want_visible_images(id);
    assert_eq!(s.jobs.len(), MAX_IN_FLIGHT_IMAGES);
    let key = match &s.jobs[0] {
        Job::Image { key } => key.clone(),
        _ => unreachable!(),
    };
    s.image_ready(key, None);
    s.want_visible_images(id);
    assert_eq!(s.jobs.len(), MAX_IN_FLIGHT_IMAGES + 1, "the next window image is queued");
    assert_eq!(s.images.pending_len(), MAX_IN_FLIGHT_IMAGES);
}

#[test]
fn a_failed_image_is_not_retried() {
    let mut s = AppState::new();
    let path = PathBuf::from("/missing.png");
    s.want_images(vec![path.clone()]);
    assert_eq!(s.images.pending_len(), 1);
    s.image_ready(path.clone(), None);
    assert_eq!(s.images.pending_len(), 0, "the failure frees the slot");
    assert!(s.images.failed(&path));
    s.jobs.clear();
    s.want_images(vec![path.clone()]);
    assert!(s.jobs.is_empty(), "a known failure is not queued again");
}

#[test]
fn a_decoded_image_is_not_queued_again() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("p.png");
    image::RgbaImage::new(2, 2).save(&path).unwrap();
    let (img, bytes) = crate::images::decode(&path, crate::images::MAX_DECODE_WIDTH).unwrap();
    let mut s = AppState::new();
    s.images.insert(path.clone(), img, bytes);
    s.want_images(vec![path]);
    assert!(s.jobs.is_empty(), "a stable window does not decode a picture again");
}

#[test]
fn closing_a_tab_forgets_its_web_images() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = AppState::new();
    let a = ready_tab(&mut s, &dir.path().join("a.md"), "a");
    let b = ready_tab(&mut s, &dir.path().join("b.md"), "b");
    let c = ready_tab(&mut s, &dir.path().join("c.md"), "c");
    for id in [a, b, c] {
        s.web_allowed.insert(id);
        s.web_urls.insert(id, vec!["https://example.com/x.png".into()]);
    }
    s.close_now(a);
    assert!(!s.web_allowed.contains(&a) && !s.web_urls.contains_key(&a));
    // Close the tabs to the right of `b`: `c` goes, `b` keeps its entries.
    s.close_clean(|i, at| i > at, b);
    assert!(!s.web_allowed.contains(&c) && !s.web_urls.contains_key(&c));
    assert!(s.web_allowed.contains(&b) && s.web_urls.contains_key(&b));
}
