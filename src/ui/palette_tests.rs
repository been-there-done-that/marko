use rune::{Env, Key, KeyEvent, Mods, Runtime};

use crate::app::AppState;
use crate::ui::keys::on_key;
use crate::ui::view;

fn runtime(state: AppState) -> Runtime<AppState> {
    let mut rt = Runtime::new(state, view, (1100.0, 760.0), Env { system_dark: false, scale: 2.0 });
    rt.on_key = Some(on_key);
    rt.frame(1.0 / 60.0);
    rt
}

fn send(rt: &mut Runtime<AppState>, key: Key, mods: Mods) {
    rt.key(KeyEvent { key, mods });
    rt.frame(1.0 / 60.0);
    rt.frame(1.0 / 60.0);
}

fn primary() -> Mods {
    Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), ..Mods::default() }
}

fn open_palette(rt: &mut Runtime<AppState>) {
    send(rt, Key::Command('p'), Mods { shift: true, ..primary() });
}

fn type_text(rt: &mut Runtime<AppState>, text: &str) {
    send(rt, Key::Text(text.into()), Mods::default());
}

#[test]
fn ctrl_shift_p_and_ctrl_k_open_the_palette() {
    let mut rt = runtime(AppState::new());
    assert!(rt.find("palette").is_none());
    open_palette(&mut rt);
    assert!(rt.state.palette.open);
    assert!(rt.find("palette").is_some() && rt.find("palette-input").is_some());
    assert!(rt.find("palette-row-0").is_some());
    send(&mut rt, Key::Escape, Mods::default());
    assert!(!rt.state.palette.open);
    send(&mut rt, Key::Command('k'), primary());
    assert!(rt.state.palette.open);
}

#[test]
fn typing_filters_and_enter_runs_the_best_match() {
    let mut rt = runtime(AppState::new());
    open_palette(&mut rt);
    type_text(&mut rt, "wide");
    assert_eq!(rt.state.palette.query.text, "wide");
    send(&mut rt, Key::Enter, Mods::default());
    assert!(rt.state.wide);
    assert!(!rt.state.palette.open);
}

#[test]
fn arrows_move_the_highlight_and_stay_in_range() {
    let mut rt = runtime(AppState::new());
    open_palette(&mut rt);
    send(&mut rt, Key::Up, Mods::default());
    assert_eq!(rt.state.palette.selected, 0);
    send(&mut rt, Key::Down, Mods::default());
    send(&mut rt, Key::Down, Mods::default());
    assert_eq!(rt.state.palette.selected, 2);
    type_text(&mut rt, "zoom");
    assert_eq!(rt.state.palette.selected, 0, "a new query starts at the top");
    for _ in 0..10 {
        send(&mut rt, Key::Down, Mods::default());
    }
    assert_eq!(rt.state.palette.selected, 1, "zoom in, zoom out");
}

#[test]
fn escape_in_the_field_closes_it() {
    let mut rt = runtime(AppState::new());
    open_palette(&mut rt);
    type_text(&mut rt, "x");
    send(&mut rt, Key::Escape, Mods::default());
    assert!(!rt.state.palette.open);
}

#[test]
fn clicking_a_row_runs_it() {
    let mut rt = runtime(AppState::new());
    open_palette(&mut rt);
    type_text(&mut rt, "toggle sidebar");
    let (x, y) = rt.find("palette-row-0").expect("row");
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
    assert!(!rt.state.sidebar.visible);
    assert!(!rt.state.palette.open);
}

#[test]
fn reopening_clears_the_old_query() {
    let mut rt = runtime(AppState::new());
    open_palette(&mut rt);
    type_text(&mut rt, "wide");
    send(&mut rt, Key::Escape, Mods::default());
    open_palette(&mut rt);
    assert_eq!(rt.state.palette.query.text, "");
}
