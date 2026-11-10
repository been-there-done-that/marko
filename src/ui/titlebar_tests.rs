use std::fs;

use rune::{Env, Runtime};

use crate::app::AppState;
use crate::ui::view;

fn runtime(state: AppState) -> Runtime<AppState> {
    Runtime::new(state, view, (1100.0, 760.0), Env { system_dark: false, scale: 2.0 })
}

/// The drawn width of the tab for `path`, after a frame, or None when it is not open.
fn tab_width(rt: &mut Runtime<AppState>, path: &std::path::Path) -> Option<f32> {
    rt.frame(1.0 / 60.0);
    let id = rt.state.tabs.iter().find(|t| t.path == path)?.id;
    rt.frame(1.0 / 60.0);
    rt.presented(&format!("t-{id}")).map(|(r, _)| r[2])
}

#[test]
fn tabs_size_to_their_names() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let short = dir.path().join("a.md");
    let other = dir.path().join("b.md");
    let long = dir.path().join("a-much-longer-file-name-here.md");
    for p in [&short, &other, &long] {
        fs::write(p, "# Title\n").expect("a fixture file");
    }

    let mut s = AppState::new();
    s.open_paths(&[short.clone(), other.clone(), long.clone()], 0.0);
    let mut rt = runtime(s);
    let short_w = tab_width(&mut rt, &short).expect("the short tab is drawn");
    let other_w = tab_width(&mut rt, &other).expect("the other short tab is drawn");
    let long_w = tab_width(&mut rt, &long).expect("the long tab is drawn");

    assert!(short_w < long_w, "a short name ({short_w}) is narrower than a long one ({long_w})");
    assert!((short_w - other_w).abs() < 2.0, "two one-letter names are about equal: {short_w} and {other_w}");
}

fn rect(rt: &mut Runtime<AppState>, key: &str) -> Option<[f32; 4]> {
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
    rt.presented(key).map(|(r, _)| r)
}

fn with_folder() -> AppState {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures");
    let mut s = AppState::new();
    s.open_paths(&[dir.clone(), dir.join("basic.md")], 0.0);
    s
}

#[test]
fn sidebar_runs_full_height_with_its_own_header() {
    let mut rt = runtime(with_folder());
    let header = rect(&mut rt, "side-header").expect("the sidebar header is drawn");
    let bar = rect(&mut rt, "titlebar").expect("the toolbar is drawn");
    assert_eq!(header[1], 0.0, "the header is at the window's top");
    assert!((header[3] - crate::theme::BAR_H).abs() < 0.5, "header height {}", header[3]);
    assert!((header[3] - bar[3]).abs() < 0.5, "header and toolbar are the same height");
    assert!(bar[0] >= header[0] + header[2], "the toolbar starts right of the sidebar: {bar:?} vs {header:?}");
    for key in ["tb-sidebar", "tb-open"] {
        let r = rect(&mut rt, key).expect("a header button");
        assert!(r[0] + r[2] <= header[0] + header[2] + 0.5, "`{key}` sits inside the sidebar header");
    }
    let find = rect(&mut rt, "tb-find").expect("find");
    assert!(find[0] >= bar[0], "Find sits in the toolbar");
}

#[test]
fn hidden_sidebar_gives_one_full_width_band() {
    let mut s = with_folder();
    s.sidebar.visible = false;
    let mut rt = runtime(s);
    assert!(rect(&mut rt, "side-header").is_none());
    let bar = rect(&mut rt, "titlebar").expect("the band is drawn");
    assert!(bar[0] < 1.0 && bar[2] > 1000.0, "full width: {bar:?}");
    assert!(rect(&mut rt, "tb-sidebar").is_some() && rect(&mut rt, "tb-open").is_some());
}
