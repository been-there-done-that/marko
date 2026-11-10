use std::path::PathBuf;

use rune::{Env, Key, KeyEvent, Mods, Runtime};

use crate::app::{AppState, Mode};
use crate::load::load;
use crate::ui::editor;
use crate::ui::keys::on_key;
use crate::ui::view;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

/// A window with `basic.md` open and loaded, so its tab is Ready.
fn ready_state() -> AppState {
    let mut s = AppState::new();
    s.open_paths(&[fixture("basic.md")], 0.0);
    let id = s.tabs.active().map(|t| t.id).expect("a tab is open");
    s.loaded(id, load(&fixture("basic.md")));
    s
}

fn runtime(state: AppState) -> Runtime<AppState> {
    let mut rt = Runtime::new(state, view, (1100.0, 760.0), Env { system_dark: false, scale: 2.0 });
    rt.on_key = Some(on_key);
    rt
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
fn edit_segment_switches_to_editor() {
    let mut rt = runtime(ready_state());
    press(&mut rt, "mode-edit");
    assert!(matches!(rt.state.mode, Mode::Edit));
    assert!(rt.find("editor").is_some());
    assert!(rt.find("preview").is_some());
    assert!(rt.state.tabs.active().and_then(|t| t.edit.as_ref()).is_some());
    press(&mut rt, "mode-preview");
    assert!(matches!(rt.state.mode, Mode::Preview));
    assert!(rt.find("editor").is_none());
}

#[test]
fn typing_updates_preview() {
    let mut rt = runtime(ready_state());
    press(&mut rt, "mode-edit");
    // Caret at the start of the source, so the typed heading opens the document.
    if let Some(edit) = rt.state.tabs.active_mut().and_then(|t| t.edit.as_mut()) {
        edit.text.anchor = 0;
        edit.text.focus = 0;
    }
    rt.key(KeyEvent { key: Key::Text("# Zebraword\n\n".into()), mods: Mods::default() });
    for _ in 0..3 {
        rt.frame(1.0 / 60.0);
    }
    let buffer = rt.state.tabs.active().and_then(|t| t.edit.as_ref()).map(|b| b.text()).unwrap_or_default();
    assert!(buffer.starts_with("# Zebraword"), "buffer: {buffer:?}");
    // The preview renders the heading as its own node, which the source area's text is not.
    let heading = rt.accessibility_tree().nodes.iter().any(|(_, n)| {
        n.label() == Some("Zebraword") || n.value() == Some("Zebraword")
    });
    assert!(heading, "the preview shows the typed heading");
}

#[test]
fn dirty_dot_shows_after_typing() {
    let mut rt = runtime(ready_state());
    press(&mut rt, "mode-edit");
    let active = rt.state.tabs.active().expect("active tab");
    assert!(!editor::dirty(active), "not dirty when entered");
    let (x, y) = rt.find("editor").expect("editor");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.key(KeyEvent { key: Key::Text("x".into()), mods: Mods::default() });
    rt.frame(1.0 / 60.0);
    let active = rt.state.tabs.active().expect("active tab");
    assert!(editor::dirty(active), "dirty after typing");
    assert!(rt.find("status-mark").is_some(), "the tab shows a status mark in place of its close button");
}
