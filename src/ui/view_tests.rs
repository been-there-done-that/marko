use std::path::PathBuf;

use rune::{Env, FileDialogError, Runtime};

use crate::app::{AppState, MenuKind};
use crate::load::{DocState, LoadError};
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
