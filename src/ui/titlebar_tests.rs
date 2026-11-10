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
