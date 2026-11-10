use rune::{Env, Key, KeyEvent, Mods, Runtime};

use crate::app::AppState;
use crate::settings::{Settings, Startup};
use crate::ui::view;

fn runtime() -> Runtime<AppState> {
    let mut rt = Runtime::new(AppState::new(), view, (1100.0, 760.0), Env { system_dark: false, scale: 2.0 });
    rt.on_key = Some(crate::ui::keys::on_key);
    rt
}

fn press(rt: &mut Runtime<AppState>, key: &str) {
    rt.frame(1.0 / 60.0);
    let (x, y) = rt.find(key).unwrap_or_else(|| panic!("no element `{key}`"));
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    for _ in 0..3 {
        rt.frame(1.0 / 60.0);
    }
}

fn open(rt: &mut Runtime<AppState>) {
    rt.state.settings_open = true;
    rt.frame(1.0 / 60.0);
}

#[test]
fn ctrl_comma_toggles_settings() {
    let mut rt = runtime();
    rt.frame(1.0 / 60.0);
    let ev = KeyEvent { key: Key::Command(','), mods: Mods { cmd: true, ctrl: !cfg!(target_os = "macos"), ..Default::default() } };
    rt.key(ev.clone());
    rt.frame(1.0 / 60.0);
    assert!(rt.state.settings_open);
    assert!(rt.find("settings-tabs").is_some());
    rt.key(ev);
    rt.frame(1.0 / 60.0);
    assert!(!rt.state.settings_open);
}

#[test]
fn tabs_switch() {
    let mut rt = runtime();
    open(&mut rt);
    assert!(rt.find("theme-card-light").is_some());
    press(&mut rt, "settings-tabs/segment3");
    assert_eq!(rt.state.settings_tab, 3);
    assert!(rt.find("settings-reset").is_some());
    assert!(rt.find("theme-card-light").is_none());
}

#[test]
fn theme_card_sets_theme() {
    let mut rt = runtime();
    open(&mut rt);
    press(&mut rt, "theme-card-nordic");
    assert_eq!(rt.state.settings.theme, "nordic");
}

#[test]
fn accent_swatch_sets_and_default_clears_accent() {
    let mut rt = runtime();
    open(&mut rt);
    press(&mut rt, "accent-pink");
    assert_eq!(rt.state.settings.accent.as_deref(), Some("#E5489B"));
    press(&mut rt, "accent-default");
    assert_eq!(rt.state.settings.accent, None);
}

#[test]
fn reset_restores_defaults() {
    let mut rt = runtime();
    rt.state.settings.text_size = 22.0;
    rt.state.settings.startup = Startup::Empty;
    rt.state.settings.theme = "paper".into();
    rt.state.settings_tab = 3;
    open(&mut rt);
    press(&mut rt, "settings-reset");
    assert_eq!(rt.state.settings, Settings::default());
}

#[test]
fn switch_in_general_toggles_progress() {
    let mut rt = runtime();
    rt.state.settings_tab = 3;
    open(&mut rt);
    let before = rt.state.settings.show_progress;
    press(&mut rt, "progress");
    assert_ne!(rt.state.settings.show_progress, before);
}

#[test]
fn fonts_show_loading_until_loaded() {
    let mut rt = runtime();
    rt.state.settings_tab = 1;
    open(&mut rt);
    assert!(rt.find("settings-tab-text").is_some());
    assert!(!rt.state.fonts.loaded);
}

#[test]
fn dragging_a_slider_sets_text_size() {
    let mut rt = runtime();
    rt.state.settings_tab = 1;
    open(&mut rt);
    rt.frame(1.0 / 60.0);
    let (x, y) = rt.find("text-size").expect("slider");
    // The rail is centred on its key point; a press at its middle is half way: 18 pt.
    rt.pointer_down(x, y);
    rt.pointer_up(x, y);
    rt.frame(1.0 / 60.0);
    let v = rt.state.settings.text_size;
    assert!((12.0..=24.0).contains(&v) && (v - v.round()).abs() < 1e-3, "got {v}");
    assert_ne!(v, 16.0);
}

#[test]
fn background_sliders_ignore_input_without_a_picture() {
    let mut rt = runtime();
    rt.state.settings_tab = 2;
    open(&mut rt);
    let before = rt.state.settings.background.clone();
    let (x, y) = rt.find("bg-blur").expect("slider");
    rt.pointer_down(x + 30.0, y);
    rt.pointer_up(x + 30.0, y);
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.settings.background, before);
}
