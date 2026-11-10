use std::path::PathBuf;

use rune::{Env, Runtime};

use super::*;
use crate::app::AppState;
use crate::ui::view;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

/// Runs `id` through a real runtime handler so it gets a `Tx`.
fn run_in_runtime(state: AppState, id: &'static str) -> Runtime<AppState> {
    let mut rt = Runtime::new(state, view, (1100.0, 760.0), Env { system_dark: false, scale: 2.0 });
    rt.frame(1.0 / 60.0);
    rt.state.palette.show();
    rt.update(move |s, tx| run(s, id, tx));
    rt.frame(1.0 / 60.0);
    rt
}

#[test]
fn fuzzy_is_a_case_insensitive_subsequence() {
    assert!(fuzzy("tsb", "Toggle Sidebar").is_some());
    assert!(fuzzy("TSB", "toggle sidebar").is_some());
    assert!(fuzzy("zzz", "Toggle Sidebar").is_none());
    assert!(fuzzy("bs", "Toggle Sidebar").is_none(), "order matters");
    assert_eq!(fuzzy("", "Anything"), Some(0));
}

#[test]
fn fuzzy_prefers_word_starts_and_runs() {
    assert!(fuzzy("fi", "Find") > fuzzy("fi", "Open File"));
    assert!(fuzzy("side", "Toggle Sidebar") > fuzzy("side", "Wide Reading Column"));
    assert!(fuzzy("set", "Settings") > fuzzy("set", "Theme: Solar Light"));
    assert!(fuzzy("ts", "Toggle Sidebar") > fuzzy("ts", "Settings"));
}

#[test]
fn ranked_puts_the_best_first_and_caps_rows() {
    let s = AppState::new();
    assert_eq!(ranked(&s, "").len(), MAX_ROWS);
    assert_eq!(ranked(&s, "zoom in").first().map(|c| c.id.as_str()), Some("zoom-in"));
    assert!(ranked(&s, "qqqq").is_empty());
}

#[test]
fn all_lists_themes_and_recent_files() {
    let mut s = AppState::new();
    s.recent = vec![fixture("basic.md")];
    let cmds = all(&s);
    assert!(cmds.iter().any(|c| c.id == "theme:nordic" && c.label == "Theme: Nordic"));
    assert!(cmds.iter().any(|c| c.label == "Open Recent: basic.md"));
    for id in ["open-file", "open-folder", "new-file", "save", "toggle-sidebar", "find", "toggle-edit", "zoom-in", "zoom-out", "zoom-reset", "wide", "settings"] {
        assert!(cmds.iter().any(|c| c.id == id), "missing {id}");
    }
}

#[test]
fn toggles_and_zoom_commands_run() {
    let rt = run_in_runtime(AppState::new(), "toggle-sidebar");
    assert!(!rt.state.sidebar.visible);
    assert!(!rt.state.palette.open, "running closes the palette");
    let rt = run_in_runtime(AppState::new(), "wide");
    assert!(rt.state.wide);
    let rt = run_in_runtime(AppState::new(), "zoom-in");
    assert!((rt.state.zoom - 1.1).abs() < 1e-4);
    let rt = run_in_runtime(AppState::new(), "zoom-out");
    assert!((rt.state.zoom - 0.9).abs() < 1e-4);
    let mut s = AppState::new();
    s.zoom = 1.5;
    let rt = run_in_runtime(s, "zoom-reset");
    assert_eq!(rt.state.zoom, 1.0);
}

#[test]
fn settings_find_and_new_file_run() {
    assert!(run_in_runtime(AppState::new(), "settings").state.settings_open);
    let mut s = AppState::new();
    s.open_paths(&[fixture("basic.md")], 0.0);
    assert!(run_in_runtime(s, "find").state.find.open);
    let rt = run_in_runtime(AppState::new(), "new-file");
    assert_eq!(rt.state.tabs.iter().count(), 1);
    assert!(matches!(rt.state.mode, crate::app::Mode::Edit));
}

#[test]
fn edit_toggle_and_save_run() {
    let mut s = AppState::new();
    s.open_paths(&[fixture("basic.md")], 0.0);
    crate::settle(&mut s);
    let rt = run_in_runtime(s, "toggle-edit");
    assert!(matches!(rt.state.mode, crate::app::Mode::Edit));
    // Save on a clean titled tab queues a write and leaves nothing else changed.
    let rt = run_in_runtime(rt.state, "save");
    assert!(rt.state.jobs.iter().any(|j| matches!(j, crate::app::Job::Save { .. })));
}

#[test]
fn dialog_commands_run_without_changing_state() {
    // The dialogs open on the UI thread; here they only have to be requested without panicking.
    let rt = run_in_runtime(AppState::new(), "open-file");
    assert!(!rt.state.palette.open);
    let rt = run_in_runtime(AppState::new(), "open-folder");
    assert!(!rt.state.palette.open);
}

#[test]
fn theme_and_recent_commands_run() {
    let rt = run_in_runtime(AppState::new(), "theme:nordic");
    assert_eq!(rt.state.settings.theme, "nordic");
    let mut s = AppState::new();
    let path = fixture("basic.md");
    s.recent = vec![path.clone()];
    let id: &'static str = Box::leak(format!("recent:{}", path.display()).into_boxed_str());
    let rt = run_in_runtime(s, id);
    assert_eq!(rt.state.tabs.iter().count(), 1);
}
