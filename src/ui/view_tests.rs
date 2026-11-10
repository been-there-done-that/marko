use std::path::PathBuf;

use rune::{Env, FileDialogError, Key, KeyEvent, Mods, Runtime};

use crate::app::{AppState, MenuKind};
use crate::load::{load, DocState, LoadError};
use crate::ui::view;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn runtime(state: AppState) -> Runtime<AppState> {
    Runtime::new(state, view, (1100.0, 760.0), Env { system_dark: false, scale: 2.0 })
}

/// Lays the view out, then presses the element with `key` and lets the frame settle.
fn press(rt: &mut Runtime<AppState>, key: &str) {
    rt.frame(1.0 / 60.0);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element `{key}`"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..3 {
        rt.frame(1.0 / 60.0);
    }
}

#[test]
fn sidebar_button_toggles_visibility() {
    let mut rt = runtime(AppState::new());
    let before = rt.state.sidebar.visible;
    press(&mut rt, "tb-sidebar");
    assert_ne!(rt.state.sidebar.visible, before);
    press(&mut rt, "tb-sidebar");
    assert_eq!(rt.state.sidebar.visible, before);
}

#[test]
fn open_button_opens_and_closes_menu() {
    let mut rt = runtime(AppState::new());
    press(&mut rt, "tb-open");
    assert_eq!(rt.state.menu, Some(MenuKind::Open));
    assert!(rt.find("menu-open-file").is_some());
    press(&mut rt, "tb-open");
    assert_eq!(rt.state.menu, None);
    assert!(rt.find("menu-open-file").is_none());
}

#[test]
fn edit_segment_is_inert() {
    let mut rt = runtime(AppState::new());
    rt.state.open_paths(&[fixture("basic.md")], 0.0);
    let active = rt.state.tabs.active().map(|t| t.id);
    let sidebar = rt.state.sidebar.visible;
    rt.frame(1.0 / 60.0);
    assert!(rt.find("mode-edit").is_some());
    press(&mut rt, "mode-edit");
    assert_eq!(rt.state.tabs.active().map(|t| t.id), active);
    assert_eq!(rt.state.sidebar.visible, sidebar);
    assert_eq!(rt.state.menu, None);
}

#[test]
fn tab_strip_selects_and_closes() {
    let mut s = AppState::new();
    s.open_paths(&[fixture("basic.md")], 0.0);
    s.open_paths(&[fixture("hello.md")], 0.0);
    let ids: Vec<u64> = s.tabs.iter().map(|t| t.id).collect();
    let (first, second) = (ids[0], ids[1]);
    assert_eq!(s.tabs.active().map(|t| t.id), Some(second));
    let mut rt = runtime(s);
    press(&mut rt, &format!("t-{first}"));
    assert_eq!(rt.state.tabs.active().map(|t| t.id), Some(first));
    // The tab's close button is not under its `t-{id}` element, and every tab's shares the key
    // `close`. The first match is the first tab's, which is the one just activated.
    press(&mut rt, "close");
    let left: Vec<u64> = rt.state.tabs.iter().map(|t| t.id).collect();
    assert_eq!(left, vec![second]);
}

#[test]
fn dialog_answer_opens_file() {
    let mut s = AppState::new();
    s.dialog_answer(Ok(Some(fixture("basic.md"))), 0.0);
    assert_eq!(s.tabs.iter().count(), 1);
    assert!(matches!(s.jobs.as_slice(), [crate::app::Job::Load { .. }]));
    s.dialog_answer(Ok(None), 1.0);
    assert_eq!(s.tabs.iter().count(), 1);
    assert!(s.toast.is_none());
    s.dialog_answer(Err(FileDialogError("no portal".into())), 2.0);
    assert_eq!(s.toast.as_ref().map(|t| t.text.as_str()), Some("Couldn't open the file dialog"));
}



#[test]
fn menu_row_key_on_lead_selects_the_item() {
    let mut s = AppState::new();
    s.open_paths(&[fixture("basic.md")], 0.0);
    assert_eq!(s.recent.len(), 1);
    let mut rt = runtime(s);
    press(&mut rt, "tb-open");
    press(&mut rt, "menu-clear-recent");
    assert!(rt.state.recent.is_empty());
    assert_eq!(rt.state.menu, None);
}

#[test]
fn empty_state_when_no_tabs() {
    let mut rt = runtime(AppState::new());
    rt.frame(1.0 / 60.0);
    assert!(rt.find("empty").is_some());
    assert!(rt.find("doc").is_none());
}

#[test]
fn closing_last_tab_shows_empty_state() {
    let mut s = AppState::new();
    s.open_paths(&[fixture("basic.md")], 0.0);
    let mut rt = runtime(s);
    press(&mut rt, "close");
    rt.frame(1.0 / 60.0);
    assert!(rt.state.tabs.is_empty());
    assert!(rt.find("empty").is_some());
}

#[test]
fn drop_mixed_paths() {
    let mut rt = runtime(AppState::new());
    rt.frame(1.0 / 60.0);
    rt.drop_files(vec![fixtures_dir(), fixture("basic.md"), fixture("picture.png")], Some((560.0, 420.0)));
    for _ in 0..3 {
        rt.frame(1.0 / 60.0);
    }
    assert_eq!(rt.state.sidebar.roots.len(), 1);
    assert_eq!(rt.state.tabs.iter().count(), 1);
    assert!(rt.find("toast").is_some());
}

#[test]
fn drop_hover_shows_overlay() {
    let mut rt = runtime(AppState::new());
    rt.state.drop_hover = true;
    rt.frame(1.0 / 60.0);
    assert!(rt.find("drop-overlay").is_some());
    rt.update(|s, _| s.drop_hover = false);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("drop-overlay").is_none());
}

#[test]
fn error_doc_shows_message() {
    let mut s = AppState::new();
    s.open_paths(&[fixture("basic.md")], 0.0);
    let id = s.tabs.active().map(|t| t.id).unwrap_or_default();
    s.loaded(id, DocState::Error(LoadError::Binary));
    let mut rt = runtime(s);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("doc-error").is_some());
}

#[test]
fn loose_files_section_only_for_files_outside_roots() {
    let mut s = AppState::new();
    let _ = s.sidebar.add_root(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src"));
    s.open_paths(&[fixture("basic.md")], 0.0);
    let mut rt = runtime(s);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("side-loose").is_some());

    let mut s = AppState::new();
    let _ = s.sidebar.add_root(&fixtures_dir());
    s.open_paths(&[fixture("basic.md")], 0.0);
    let mut rt = runtime(s);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("side-loose").is_none());
}

#[test]
fn tab_context_menu_close_others() {
    let mut s = AppState::new();
    s.open_paths(&[fixture("basic.md")], 0.0);
    s.open_paths(&[fixture("hello.md")], 0.0);
    s.open_paths(&[fixture("frontmatter.md")], 0.0);
    let first = s.tabs.iter().next().map(|t| t.id).unwrap_or_default();
    let mut rt = runtime(s);
    rt.frame(1.0 / 60.0);
    let (x, y) = rt.find(&format!("t-{first}")).expect("first tab");
    rt.secondary_down(x, y);
    rt.secondary_up(x, y);
    for _ in 0..3 {
        rt.frame(1.0 / 60.0);
    }
    assert_eq!(rt.state.menu, Some(MenuKind::TabContext { id: first }));
    press(&mut rt, "tabmenu-others");
    let left: Vec<u64> = rt.state.tabs.iter().map(|t| t.id).collect();
    assert_eq!(left, vec![first]);
    assert_eq!(rt.state.menu, None);
}

#[test]
fn more_menu_copy_source_copies_markdown() {
    let mut s = AppState::new();
    s.open_paths(&[fixture("basic.md")], 0.0);
    let id = s.tabs.active().map(|t| t.id).unwrap_or_default();
    let src = "# Title\n\nBody *text*.\n";
    s.loaded(id, DocState::Ready { src: src.into(), large: false, lossy: false });
    let mut rt = runtime(s);
    let clipboard = rune::clipboard::memory();
    rt.use_clipboard(&clipboard);
    press(&mut rt, "tb-more");
    press(&mut rt, "menu-copy-source");
    assert_eq!(clipboard.text().as_deref().map(String::as_str), Some(src));
    assert_eq!(rt.state.menu, None);
}

#[test]
fn try_again_requeues_load() {
    let mut s = AppState::new();
    s.open_paths(&[fixture("basic.md")], 0.0);
    let id = s.tabs.active().map(|t| t.id).unwrap_or_default();
    s.loaded(id, DocState::Error(LoadError::Binary));
    s.jobs.clear();
    let mut rt = runtime(s);
    press(&mut rt, "doc-retry");
    assert!(rt.state.jobs.iter().any(|j| matches!(j, crate::app::Job::Load { tab, .. } if *tab == id)));
    assert!(matches!(rt.state.tabs.active().map(|t| &t.doc), Some(DocState::Loading)));
}

/// Cmd (Ctrl off macOS) plus `c`, as winit reports it.
fn cmd_key(c: char) -> KeyEvent {
    let mods = Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), ..Mods::default() };
    KeyEvent { key: Key::Command(c), mods }
}

/// A window with `basic.md` open, loaded and edited, so its tab is dirty.
fn dirty_runtime() -> Runtime<AppState> {
    let mut s = AppState::new();
    s.open_paths(&[fixture("basic.md")], 0.0);
    let id = s.tabs.active().map(|t| t.id).expect("a tab is open");
    s.loaded(id, load(&fixture("basic.md")));
    crate::ui::editor::enter(&mut s);
    s.tabs.active_mut().and_then(|t| t.edit.as_mut()).expect("a buffer").text.replace_text("changed\n");
    let mut rt = runtime(s);
    rt.on_key = Some(crate::ui::keys::on_key);
    rt.frame(1.0 / 60.0);
    rt
}

#[test]
fn close_dirty_tab_asks() {
    let mut rt = dirty_runtime();
    rt.key(cmd_key('w'));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.tabs.iter().count(), 1, "the tab stays open");
    assert!(rt.state.guard.is_some());
    assert!(rt.find("guard-save").is_some());
    assert!(rt.find("guard-discard").is_some());
    assert!(rt.find("guard-cancel").is_some());
}

#[test]
fn guard_discard_closes() {
    let mut rt = dirty_runtime();
    rt.key(cmd_key('w'));
    rt.frame(1.0 / 60.0);
    press(&mut rt, "guard-discard");
    assert!(rt.state.tabs.is_empty());
    assert!(rt.state.guard.is_none());
    assert!(rt.find("guard-save").is_none());
}

#[test]
fn guard_cancel_keeps_tab() {
    let mut rt = dirty_runtime();
    rt.key(cmd_key('w'));
    rt.frame(1.0 / 60.0);
    press(&mut rt, "guard-cancel");
    assert_eq!(rt.state.tabs.iter().count(), 1);
    assert!(rt.state.guard.is_none());
    assert!(rt.find("guard-save").is_none());
}

#[test]
fn guard_save_writes_then_closes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("b.md");
    std::fs::write(&path, "old\n").unwrap();
    let mut s = AppState::new();
    s.open_paths(std::slice::from_ref(&path), 0.0);
    let id = s.tabs.active().map(|t| t.id).expect("a tab is open");
    s.loaded(id, load(&path));
    crate::ui::editor::enter(&mut s);
    s.tabs.active_mut().and_then(|t| t.edit.as_mut()).expect("a buffer").text.replace_text("new\n");
    let mut rt = runtime(s);
    rt.on_key = Some(crate::ui::keys::on_key);
    rt.key(cmd_key('w'));
    rt.frame(1.0 / 60.0);
    rt.state.jobs.clear();
    press(&mut rt, "guard-save");
    assert_eq!(rt.state.tabs.iter().count(), 1, "the tab waits for its write");
    let jobs: Vec<crate::app::Job> = rt.state.jobs.drain(..).collect();
    for job in jobs {
        crate::worker::run_job(job).apply(&mut rt.state);
    }
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
    assert!(rt.state.tabs.is_empty());
    assert!(rt.state.guard.is_none());
}

#[test]
fn close_others_keeps_dirty_tabs() {
    let dir = tempfile::tempdir().unwrap();
    let paths: Vec<PathBuf> = ["a.md", "b.md", "c.md"].iter().map(|n| { let p = dir.path().join(n); std::fs::write(&p, "# x\n").unwrap(); p }).collect();
    let mut s = AppState::new();
    s.open_paths(&paths, 0.0);
    for job in std::mem::take(&mut s.jobs) { crate::worker::run_job(job).apply(&mut s); }
    let ids: Vec<u64> = s.tabs.iter().map(|t| t.id).collect();
    let mut buf = crate::edit::EditBuffer::from_src("# x\n");
    buf.text = rune::TextEdit::new("# changed\n");
    s.tabs.get_mut(ids[0]).unwrap().edit = Some(buf);
    s.close_clean(|i, at| i != at, ids[2]);
    let left: Vec<u64> = s.tabs.iter().map(|t| t.id).collect();
    assert_eq!(left, vec![ids[0], ids[2]], "the dirty tab survives, the clean one closes");
}
