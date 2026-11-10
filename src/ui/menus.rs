//! The Open menu (§4.4) and the More menu (§4.5), both popovers under their buttons.

use std::path::Path;

use rune::{Color, El, FileDialogOptions, Icon, Tx, el, icon};
use rune_kit::{Kit, Menu, MenuItem};

use crate::app::{AppState, Appearance, MenuKind};
use crate::icons;
use crate::theme;
use crate::ui::titlebar::chord;

/// Recent entries shown in the Open menu. The state keeps more.
const RECENT_SHOWN: usize = 8;
const MENU_MIN_W: f32 = 240.0;
/// Zoom steps and limits. Zoom multiplies the reading sizes (§4.7).
const ZOOM_STEP: f32 = 0.1;
const ZOOM_MIN: f32 = 0.5;
const ZOOM_MAX: f32 = 2.0;
/// Extensions the Open File dialog offers (§4.4).
const MARKDOWN_EXTS: &[&str] = &["md", "markdown", "mdown", "mkd", "txt"];

/// The 14pt slot at the start of a row. Every row has one so labels line up, and the slot
/// carries the row's key: a menu item takes no key of its own.
fn lead(key: impl Into<String>, i: Option<&'static Icon>, color: Color) -> El {
    let key = key.into();
    match i {
        Some(i) => icon(i, 14.0).key(key).color(color),
        None => el().key(key).size(14.0, 14.0),
    }
}

fn zoom_by(zoom: f32, delta: f32) -> f32 {
    (zoom + delta).clamp(ZOOM_MIN, ZOOM_MAX)
}

fn open_file(s: &mut AppState, tx: &mut Tx) {
    s.menu = None;
    let options = FileDialogOptions { title: "Open Markdown".into(), ..Default::default() }.filter("Markdown", MARKDOWN_EXTS);
    tx.open_file(options, |s: &mut AppState, r, _: &mut Tx| {
        let now = s.now;
        s.dialog_answer(r, now);
    });
}

fn open_folder(s: &mut AppState, tx: &mut Tx) {
    s.menu = None;
    let options = FileDialogOptions { title: "Open Folder".into(), ..Default::default() };
    tx.open_folder(options, |s: &mut AppState, r, _: &mut Tx| {
        let now = s.now;
        s.dialog_answer(r, now);
    });
}

/// Opens a Recent entry. One that no longer exists is dropped from the list instead.
fn pick_recent(s: &mut AppState, path: &Path) {
    s.menu = None;
    if path.exists() {
        let now = s.now;
        s.open_paths(&[path.to_path_buf()], now);
    } else {
        s.recent.retain(|p| p != path);
    }
}

/// The Open menu under its button. Its entries are pushed into `layers` when it is open.
pub fn open_menu(s: &AppState, k: &Kit, trigger: El, layers: &mut Vec<El>) -> El {
    let t = theme::tokens(k.dark);
    let mut menu = Menu::new("menu-open")
        .trigger(trigger)
        .open(s.menu == Some(MenuKind::Open))
        .min_width(MENU_MIN_W)
        .on_close(|s: &mut AppState| s.menu = None)
        .item(
            MenuItem::new("New Markdown File")
                .lead(lead("menu-new", Some(&icons::FILE_PLUS), t.fg2))
                .badge("Phase 2")
                .disabled(true),
        )
        .separator()
        .item(
            MenuItem::new("Open File…")
                .lead(lead("menu-open-file", Some(&icons::FILE), t.fg2))
                .shortcut(chord(false, "O"))
                .on_select_tx(open_file),
        )
        .item(
            MenuItem::new("Open Folder…")
                .lead(lead("menu-open-folder", Some(&icons::FOLDER_OPEN), t.fg2))
                .shortcut(chord(true, "O"))
                .on_select_tx(open_folder),
        );
    if !s.recent.is_empty() {
        menu = menu.separator().group_label("Recent");
        for (i, path) in s.recent.iter().take(RECENT_SHOWN).enumerate() {
            let icon_for = if path.is_dir() { &icons::FOLDER } else { &icons::FILE_TEXT };
            let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
            let parent = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
            let target = path.clone();
            menu = menu.item(
                MenuItem::new(name)
                    .lead(lead(format!("menu-recent-{i}"), Some(icon_for), t.fg2))
                    .description(parent)
                    .on_select_tx(move |s: &mut AppState, _: &mut Tx| pick_recent(s, &target)),
            );
        }
        menu = menu.separator().item(
            MenuItem::new("Clear Recent")
                .lead(lead("menu-clear-recent", Some(&icons::TRASH_2), t.fg2))
                .on_select(|s: &mut AppState| {
                    s.recent.clear();
                    s.menu = None;
                }),
        );
    }
    menu.build(k, layers)
}

fn copy_path(s: &mut AppState, tx: &mut Tx) {
    s.menu = None;
    if let Some(tab) = s.tabs.active() {
        tx.copy(tab.path.display().to_string());
    }
}

fn close_tab(s: &mut AppState) {
    s.menu = None;
    if let Some(id) = s.tabs.active().map(|tab| tab.id) {
        s.tabs.close(id);
    }
}

fn appearance_item(s: &AppState, label: &'static str, want: Appearance) -> MenuItem {
    MenuItem::new(label).checked(s.appearance == want).on_select(move |s: &mut AppState| {
        s.appearance = want;
        s.menu = None;
    })
}

/// The More menu under its button. Its entries are pushed into `layers` when it is open.
pub fn more_menu(s: &AppState, k: &Kit, trigger: El, layers: &mut Vec<El>) -> El {
    let t = theme::tokens(k.dark);
    let has_tab = s.tabs.active().is_some();
    Menu::new("menu-more")
        .trigger(trigger)
        .open(s.menu == Some(MenuKind::More))
        .min_width(MENU_MIN_W)
        .on_close(|s: &mut AppState| s.menu = None)
        .item(
            MenuItem::new("Copy Path")
                .lead(lead("menu-copy-path", Some(&icons::COPY), t.fg2))
                .shortcut(chord(true, "C"))
                .disabled(!has_tab)
                .on_select_tx(copy_path),
        )
        .separator()
        .item(
            MenuItem::new("Zoom In")
                .lead(lead("menu-zoom-in", Some(&icons::ZOOM_IN), t.fg2))
                .shortcut(chord(false, "+"))
                .on_select(|s: &mut AppState| {
                    s.zoom = zoom_by(s.zoom, ZOOM_STEP);
                    s.menu = None;
                }),
        )
        .item(
            MenuItem::new("Zoom Out")
                .lead(lead("menu-zoom-out", Some(&icons::ZOOM_OUT), t.fg2))
                .shortcut(chord(false, "−"))
                .on_select(|s: &mut AppState| {
                    s.zoom = zoom_by(s.zoom, -ZOOM_STEP);
                    s.menu = None;
                }),
        )
        .item(
            MenuItem::new("Actual Size")
                .lead(lead("menu-zoom-reset", None, t.fg2))
                .shortcut(chord(false, "0"))
                .on_select(|s: &mut AppState| {
                    s.zoom = 1.0;
                    s.menu = None;
                }),
        )
        .separator()
        .item(
            MenuItem::new("Wide Reading Column")
                .lead(lead("menu-wide", None, t.fg2))
                .checked(s.wide)
                .on_select(|s: &mut AppState| {
                    s.wide = !s.wide;
                    s.menu = None;
                }),
        )
        .separator()
        .group_label("Appearance")
        .item(appearance_item(s, "System", Appearance::System).lead(lead("menu-appearance-system", None, t.fg2)))
        .item(appearance_item(s, "Light", Appearance::Light).lead(lead("menu-appearance-light", None, t.fg2)))
        .item(appearance_item(s, "Dark", Appearance::Dark).lead(lead("menu-appearance-dark", None, t.fg2)))
        .separator()
        .item(
            MenuItem::new("Close Tab")
                .lead(lead("menu-close-tab", Some(&icons::X), t.fg2))
                .shortcut(chord(false, "W"))
                .disabled(!has_tab)
                .on_select(close_tab),
        )
        .build(k, layers)
}
