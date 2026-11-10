use std::path::PathBuf;

use rune::{Env, Key, KeyEvent, Mods, Runtime};

use crate::app::{AppState, Job, MenuKind};
use crate::load::DocState;
use crate::ui::keys::on_key;
use crate::ui::view;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

fn runtime(state: AppState) -> Runtime<AppState> {
    let mut rt = Runtime::new(state, view, (1100.0, 760.0), Env { system_dark: false, scale: 2.0 });
    rt.on_key = Some(on_key);
    rt.frame(1.0 / 60.0);
    rt
}

/// Queues a key press and lets the frame that handles it run.
fn send(rt: &mut Runtime<AppState>, ev: KeyEvent) {
    rt.key(ev);
    rt.frame(1.0 / 60.0);
}

/// ⌘ on macOS, Ctrl elsewhere, as winit reports it.
fn primary() -> Mods {
    Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), ..Mods::default() }
}

fn cmd(c: char) -> KeyEvent {
    KeyEvent { key: Key::Command(c), mods: primary() }
}

/// Ctrl+Tab, or Ctrl+Shift+Tab when `shift` is set.
fn ctrl_tab(shift: bool) -> KeyEvent {
    KeyEvent { key: Key::Tab, mods: Mods { ctrl: true, shift, ..primary() } }
}

#[test]
fn cmd_w_closes_active_tab() {
    let mut rt = runtime(AppState::new());
    rt.state.open_paths(&[fixture("basic.md"), fixture("frontmatter.md")], 0.0);
    let active = rt.state.tabs.active().map(|t| t.id).unwrap();
    send(&mut rt, cmd('w'));
    assert_eq!(rt.state.tabs.iter().count(), 1);
    assert!(rt.state.tabs.iter().all(|t| t.id != active));
}

#[test]
fn cmd_b_toggles_sidebar() {
    let mut rt = runtime(AppState::new());
    let before = rt.state.sidebar.visible;
    send(&mut rt, cmd('b'));
    assert_ne!(rt.state.sidebar.visible, before);
    send(&mut rt, cmd('b'));
    assert_eq!(rt.state.sidebar.visible, before);
}

#[test]
fn cmd_9_activates_last() {
    let mut rt = runtime(AppState::new());
    rt.state.open_paths(&[fixture("basic.md"), fixture("frontmatter.md"), fixture("hello.md")], 0.0);
    let first = rt.state.tabs.iter().next().map(|t| t.id).unwrap();
    let last = rt.state.tabs.iter().last().map(|t| t.id).unwrap();
    rt.state.tabs.activate(first);
    send(&mut rt, cmd('9'));
    assert_eq!(rt.state.tabs.active().map(|t| t.id), Some(last));
}

#[test]
fn ctrl_tab_cycles() {
    let mut rt = runtime(AppState::new());
    rt.state.open_paths(&[fixture("basic.md"), fixture("frontmatter.md")], 0.0);
    let first = rt.state.tabs.iter().next().map(|t| t.id).unwrap();
    let last = rt.state.tabs.iter().last().map(|t| t.id).unwrap();
    send(&mut rt, ctrl_tab(false));
    assert_eq!(rt.state.tabs.active().map(|t| t.id), Some(first), "forward wraps to the first tab");
    send(&mut rt, ctrl_tab(true));
    assert_eq!(rt.state.tabs.active().map(|t| t.id), Some(last), "Ctrl+Shift+Tab goes back, wrapping to the last");
}

#[test]
fn zoom_clamps() {
    let mut rt = runtime(AppState::new());
    for _ in 0..20 {
        send(&mut rt, cmd('+'));
    }
    assert_eq!(rt.state.zoom, 2.0);
    for _ in 0..30 {
        send(&mut rt, cmd('-'));
    }
    assert_eq!(rt.state.zoom, 0.6);
    send(&mut rt, cmd('0'));
    assert_eq!(rt.state.zoom, 1.0);
}

#[test]
fn escape_closes_menu() {
    let mut rt = runtime(AppState::new());
    rt.state.menu = Some(MenuKind::Open);
    send(&mut rt, KeyEvent { key: Key::Escape, mods: Mods::default() });
    assert_eq!(rt.state.menu, None);
}

#[test]
fn cmd_r_reloads_active_tab() {
    let mut rt = runtime(AppState::new());
    rt.state.open_paths(&[fixture("basic.md")], 0.0);
    rt.state.jobs.clear();
    let id = rt.state.tabs.active().map(|t| t.id).unwrap();
    send(&mut rt, cmd('r'));
    assert!(rt.state.jobs.iter().any(|j| matches!(j, Job::Load { tab, .. } if *tab == id)));
    assert!(matches!(rt.state.tabs.active().map(|t| &t.doc), Some(DocState::Loading)));
}

#[test]
fn bare_r_does_nothing() {
    let mut rt = runtime(AppState::new());
    rt.state.open_paths(&[fixture("basic.md")], 0.0);
    rt.state.jobs.clear();
    send(&mut rt, KeyEvent { key: Key::Text("r".into()), mods: Mods::default() });
    assert!(rt.state.jobs.is_empty());
}

/// Opens `basic.md` and loads it straight away, as the worker would.
fn loaded_basic(rt: &mut Runtime<AppState>) -> u64 {
    rt.state.open_paths(&[fixture("basic.md")], 0.0);
    rt.state.jobs.clear();
    let id = rt.state.tabs.active().map(|t| t.id).unwrap();
    rt.state.loaded(id, crate::load::load(&fixture("basic.md")));
    rt.frame(1.0 / 60.0);
    id
}

#[test]
fn cmd_f_opens_find_bar() {
    let mut rt = runtime(AppState::new());
    loaded_basic(&mut rt);
    assert!(rt.find("find-bar").is_none());
    send(&mut rt, cmd('f'));
    assert!(rt.state.find.open);
    assert!(rt.find("find-bar").is_some());
    assert!(rt.find("find-input").is_some());
}

#[test]
fn escape_closes_find_and_clears() {
    let mut rt = runtime(AppState::new());
    loaded_basic(&mut rt);
    send(&mut rt, cmd('f'));
    rt.state.find.query = rune::TextEdit::new("alpha");
    rt.state.sync_find();
    assert!(rt.state.find.count() > 0);
    send(&mut rt, KeyEvent { key: Key::Escape, mods: Mods::default() });
    assert!(!rt.state.find.open);
    assert_eq!(rt.state.find.count(), 0);
    assert!(rt.find("find-bar").is_none());
}

#[test]
fn enter_and_shift_enter_step_matches() {
    let mut rt = runtime(AppState::new());
    loaded_basic(&mut rt);
    send(&mut rt, cmd('f'));
    rt.state.find.query = rune::TextEdit::new("e");
    rt.state.sync_find();
    let n = rt.state.find.count();
    assert!(n >= 2, "basic.md has several blocks with an e, got {n}");
    send(&mut rt, KeyEvent { key: Key::Enter, mods: Mods::default() });
    assert_eq!(rt.state.find.current, 1);
    send(&mut rt, KeyEvent { key: Key::Enter, mods: Mods { shift: true, ..Mods::default() } });
    assert_eq!(rt.state.find.current, 0);
}

#[test]
fn find_follows_the_active_tab() {
    let mut rt = runtime(AppState::new());
    let first = loaded_basic(&mut rt);
    send(&mut rt, cmd('f'));
    rt.state.find.query = rune::TextEdit::new("alpha");
    rt.state.sync_find();
    assert_eq!(rt.state.find.for_tab, Some(first));
    rt.state.open_paths(&[fixture("hello.md")], 0.0);
    rt.state.jobs.clear();
    let second = rt.state.tabs.active().map(|t| t.id).unwrap();
    rt.state.loaded(second, crate::load::load(&fixture("hello.md")));
    send(&mut rt, ctrl_tab(false));
    assert_eq!(rt.state.tabs.active().map(|t| t.id), Some(first));
    assert_eq!(rt.state.find.for_tab, Some(first));
    assert_eq!(rt.state.find.label(), format!("1 of {}", rt.state.find.count()));
}
