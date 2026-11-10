use super::*;
use std::path::Path;
use crate::app::AppState;
use crate::load::DocState;
use crate::paths::canonical;
use crate::worker::run_job;

/// Writes `name` into `dir` and returns its canonical path.
fn write(dir: &Path, name: &str, text: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    canonical(&p)
}

/// Runs every queued job on this thread, as the worker would.
fn settle(s: &mut AppState) {
    for job in std::mem::take(&mut s.jobs) {
        run_job(job).apply(s);
    }
}

fn src_of(s: &mut AppState, id: u64) -> String {
    match &s.tabs.get_mut(id).unwrap().doc {
        DocState::Ready { src, .. } => src.to_string(),
        _ => panic!("expected a ready document"),
    }
}

#[test]
fn debounce_waits_150ms() {
    let p = PathBuf::from("/notes/a.md");
    let mut events = vec![(p.clone(), 0.0)];
    assert!(debounce(&mut events, 0.1).is_empty());
    assert_eq!(events.len(), 1);
    assert_eq!(debounce(&mut events, 0.15), vec![p]);
    assert!(events.is_empty());
}

#[test]
fn rename_over_is_change() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(&canonical(dir.path()), "a.md", "# one");
    let (tx, rx) = std::sync::mpsc::channel();
    let mut w = Watcher::start(move |ev| {
        let _ = tx.send(ev);
    })
    .unwrap();
    w.sync(std::slice::from_ref(&file), &[]);

    // An editor's atomic save: write a temp file, then rename it over the original.
    let tmp = file.with_file_name("a.md.tmp");
    std::fs::write(&tmp, "# two").unwrap();
    std::fs::rename(&tmp, &file).unwrap();

    let first = rx.recv_timeout(Duration::from_secs(2)).expect("a change within 2 s");
    assert_eq!(first, FileEvent::Changed(file.clone()));
    assert!(rx.recv_timeout(Duration::from_millis(500)).is_err(), "one change, not several");
}

#[test]
fn same_content_does_not_toast() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(&canonical(dir.path()), "a.md", "# same");
    let mut s = AppState::new();
    s.open_paths(std::slice::from_ref(&file), 0.0);
    settle(&mut s);
    let id = s.tabs.active().unwrap().id;

    // A save that wrote the same bytes: nothing visible changes.
    s.file_event(FileEvent::Changed(file.clone()));
    settle(&mut s);
    assert!(s.toast.is_none());
    assert!(!s.tabs.active().unwrap().changed);

    // A real change to the active tab: a toast, and no dot.
    std::fs::write(&file, "# different").unwrap();
    s.file_event(FileEvent::Changed(file.clone()));
    settle(&mut s);
    assert_eq!(s.toast.as_ref().unwrap().text, "Reloaded a.md");
    assert!(!s.tabs.active().unwrap().changed);
    assert_eq!(src_of(&mut s, id), "# different");
}

#[test]
fn deleted_then_restored() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(&canonical(dir.path()), "a.md", "# kept");
    let mut s = AppState::new();
    s.open_paths(std::slice::from_ref(&file), 0.0);
    settle(&mut s);
    let id = s.tabs.active().unwrap().id;

    std::fs::remove_file(&file).unwrap();
    s.file_event(FileEvent::Removed(file.clone()));
    assert!(s.tabs.active().unwrap().deleted);
    assert_eq!(s.toast.as_ref().unwrap().text, "a.md was deleted");
    assert_eq!(src_of(&mut s, id), "# kept", "the last content stays on screen");

    std::fs::write(&file, "# back").unwrap();
    s.file_event(FileEvent::Changed(file.clone()));
    settle(&mut s);
    assert!(!s.tabs.active().unwrap().deleted);
    assert_eq!(src_of(&mut s, id), "# back");
}
