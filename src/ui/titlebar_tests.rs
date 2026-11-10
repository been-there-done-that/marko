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
fn tabs_shrink_to_the_longest_name() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let short = dir.path().join("a.md");
    let other = dir.path().join("b.md");
    let long = dir.path().join("a-much-longer-file-name-here.md");
    for p in [&short, &other, &long] {
        fs::write(p, "# Title\n").expect("a fixture file");
    }

    let mut few = AppState::new();
    few.open_paths(&[short.clone(), other.clone()], 0.0);
    let mut rt = runtime(few);
    let short_w = tab_width(&mut rt, &short).expect("the short tab is drawn");

    let mut many = AppState::new();
    many.open_paths(&[short.clone(), other.clone(), long.clone()], 0.0);
    let mut rt = runtime(many);
    let with_long_w = tab_width(&mut rt, &short).expect("the short tab is drawn");

    assert!(short_w < with_long_w, "short names: {short_w} should be narrower than {with_long_w}");
}
