use std::path::PathBuf;

use rune::{Env, Key, KeyEvent, Mods, Runtime};

use crate::app::{AppState, Mode};
use crate::load::{load, DocState};
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

/// Edits the active tab in Edit mode, then lets the disk change under it: the reload asks
/// instead of replacing the text, and the banner shows.
fn conflicted() -> Runtime<AppState> {
    let mut rt = runtime(ready_state());
    press(&mut rt, "mode-edit");
    let id = rt.state.tabs.active().map(|t| t.id).expect("active tab");
    if let Some(buf) = rt.state.tabs.active_mut().and_then(|t| t.edit.as_mut()) {
        buf.text.text.push_str(" edited");
    }
    let changed = DocState::Ready { src: "# changed on disk".into(), large: false, lossy: false };
    rt.update(|s, _| s.loaded(id, changed));
    rt.frame(1.0 / 60.0);
    assert!(rt.find("conflict-banner").is_some(), "the banner shows");
    rt
}

#[test]
fn conflict_banner_keep_mine() {
    let mut rt = conflicted();
    press(&mut rt, "conflict-keep");
    assert!(rt.find("conflict-banner").is_none(), "the banner goes");
    let tab = rt.state.tabs.active().expect("active tab");
    assert!(!tab.conflict);
    assert!(editor::dirty(tab), "the edits are kept for the next save");
}

#[test]
fn conflict_banner_reload_discards() {
    let mut rt = conflicted();
    press(&mut rt, "conflict-reload");
    assert!(rt.find("conflict-banner").is_none(), "the banner goes");
    let tab = rt.state.tabs.active().expect("active tab");
    assert!(!tab.conflict);
    assert!(tab.edit.is_none(), "the edits are discarded");
    assert!(rt.state.jobs.iter().any(|j| matches!(j, crate::app::Job::Load { .. })), "the file is loaded again");
}

#[test]
fn editor_style_follows_the_code_font() {
    use crate::settings::Settings;
    let settings = Settings { code_font: "DejaVu Sans Mono".into(), ligatures: true, text_size: 16.0, ..Settings::default() };
    let look = crate::theme::look(&settings, &[], false);
    let style = editor::editor_style(look, &settings, 1.0);
    assert!(matches!(style.family, rune::Family::Named("DejaVu Sans Mono")));
    assert!(style.features.contains("'liga' 1"));
    assert_eq!(style.size, 13.0);
    assert!((style.line_height - 13.0 * 1.6).abs() < 0.01);
    let bigger = Settings { text_size: 20.0, ..settings.clone() };
    let zoomed = editor::editor_style(look, &bigger, 1.5);
    assert!((zoomed.size - 13.0 * 1.5 * 20.0 / 16.0).abs() < 0.01);
    let default = editor::editor_style(look, &Settings::default(), 1.0);
    assert!(!matches!(default.family, rune::Family::Named(_)));
}

#[test]
fn editor_field_still_edits_with_a_named_code_font() {
    let mut s = ready_state();
    s.settings.code_font = "DejaVu Sans Mono".into();
    let mut rt = runtime(s);
    press(&mut rt, "mode-edit");
    rt.key(KeyEvent { key: Key::Text("q".into()), mods: Mods::default() });
    rt.frame(1.0 / 60.0);
    assert!(rt.state.tabs.active().and_then(|t| t.edit.as_ref()).is_some_and(|b| b.text().contains('q')));
}
