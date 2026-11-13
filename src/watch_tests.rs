use super::*;
use std::path::Path;
use crate::app::AppState;
use crate::edit::EditBuffer;
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

/// Waits until the watcher thread has applied `n` requests, failing after 2 s.
fn wait_applied(w: &Watcher, n: usize) {
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    while w.applied() < n {
        assert!(std::time::Instant::now() < deadline, "the watch was not applied within 2 s");
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn sync_is_applied_off_the_caller_thread() {
    let dir = tempfile::tempdir().unwrap();
    let root = canonical(dir.path());
    let mut w = Watcher::start(|_| {}).unwrap();

    // Returns without waiting for the recursive watch; the thread applies it shortly after.
    w.sync(&[], std::slice::from_ref(&root));
    wait_applied(&w, 1);

    // The same lists again change nothing, so no request is sent.
    w.sync(&[], std::slice::from_ref(&root));
    std::thread::sleep(Duration::from_millis(100));
    assert_eq!(w.applied(), 1);
}

#[test]
fn rename_over_is_change() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(&canonical(dir.path()), "a.md", "# one");
    let (tx, rx) = std::sync::mpsc::channel();
    // Polling instead of inotify: the test host's watch budget can be spent by other programs,
    // and this test is about the event mapping, not the backend.
    let mut w = Watcher::start_polling(move |ev| {
        let _ = tx.send(ev);
    })
    .unwrap();
    w.sync(std::slice::from_ref(&file), &[]);
    wait_applied(&w, 1);
    // PollWatcher needs a sweep to record the file before the change is visible to it.
    std::thread::sleep(Duration::from_secs(1));

    // An editor's atomic save: write a temp file, then rename it over the original.
    let tmp = file.with_file_name("a.md.tmp");
    // A different length, so a polling backend notices even where timestamps are coarse.
    std::fs::write(&tmp, "# two, and more text").unwrap();
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

#[test]
fn dirty_tab_reload_sets_conflict() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(&canonical(dir.path()), "a.md", "# kept");
    let mut s = AppState::new();
    s.open_paths(std::slice::from_ref(&file), 0.0);
    settle(&mut s);
    let id = s.tabs.active().unwrap().id;
    {
        let tab = s.tabs.active_mut().unwrap();
        let mut buf = EditBuffer::from_src("# kept");
        buf.text.text.push_str(" edited");
        tab.edit = Some(buf);
    }

    // Another program writes the file while the tab holds unsaved edits.
    std::fs::write(&file, "# outside").unwrap();
    s.file_event(FileEvent::Changed(file.clone()));
    settle(&mut s);

    let tab = s.tabs.get_mut(id).unwrap();
    assert!(tab.conflict, "the tab asks instead of reloading");
    assert_eq!(src_of(&mut s, id), "# kept", "the document is not replaced");
    let buf = s.tabs.get_mut(id).unwrap().edit.as_ref().unwrap().text();
    assert_eq!(buf, "# kept edited", "the edits stay");
    assert!(s.toast.is_none(), "no reload toast for a conflict");
}

#[test]
fn clean_buffer_follows_reload() {
    let dir = tempfile::tempdir().unwrap();
    let file = write(&canonical(dir.path()), "a.md", "# kept");
    let mut s = AppState::new();
    s.open_paths(std::slice::from_ref(&file), 0.0);
    settle(&mut s);
    let id = s.tabs.active().unwrap().id;
    s.tabs.active_mut().unwrap().edit = Some(EditBuffer::from_src("# kept"));

    std::fs::write(&file, "# outside").unwrap();
    s.file_event(FileEvent::Changed(file.clone()));
    settle(&mut s);

    let tab = s.tabs.get_mut(id).unwrap();
    assert!(!tab.conflict);
    assert_eq!(src_of(&mut s, id), "# outside");
    let buf = s.tabs.get_mut(id).unwrap().edit.as_ref().unwrap().text();
    assert_eq!(buf, "# outside", "a clean buffer shows the reloaded text");
}

#[test]
fn atomic_save_temps_are_recognised() {
    assert!(is_atomic_temp(Path::new("/root/.a.md.123.0.tmp")));
    assert!(!is_atomic_temp(Path::new("/root/a.md")));
    assert!(!is_atomic_temp(Path::new("/root/.hidden.md")));
}

fn create_event(path: &Path) -> Event {
    Event::new(EventKind::Create(notify::event::CreateKind::File)).add_path(path.to_path_buf())
}

#[test]
fn changes_in_skipped_folders_do_not_rescan_the_root() {
    let root = PathBuf::from("/proj");
    let interest = Interest { files: Vec::new(), roots: vec![root.clone()] };
    assert!(keys_for(&create_event(Path::new("/proj/target/debug/build/x.o")), &interest).is_empty());
    assert!(keys_for(&create_event(Path::new("/proj/web/node_modules/a/README.md")), &interest).is_empty());
    assert!(keys_for(&create_event(Path::new("/proj/.git/objects/ab/cd")), &interest).is_empty());
    assert_eq!(keys_for(&create_event(Path::new("/proj/docs/new.md")), &interest), vec![root]);
    // Only components below the root count: a root that is itself hidden still rescans.
    let hidden = PathBuf::from("/home/u/.notes");
    let interest = Interest { files: Vec::new(), roots: vec![hidden.clone()] };
    assert_eq!(keys_for(&create_event(Path::new("/home/u/.notes/a.md")), &interest), vec![hidden]);
}

#[test]
fn open_file_in_skipped_folder_still_gets_its_event() {
    let file = PathBuf::from("/proj/target/doc/notes.md");
    let interest = Interest { files: vec![file.clone()], roots: vec![PathBuf::from("/proj")] };
    assert_eq!(keys_for(&create_event(&file), &interest), vec![file]);
}
