//! The Open menu (§4.4) and the More menu (§4.5), both popovers under their buttons.

use std::path::Path;

use rune::{Color, El, FileDialogOptions, Icon, PointerPress, Tx, el, icon};
use rune_kit::{Kit, Menu, MenuItem, context_menu};

use crate::app::{AppState, Appearance, MenuKind};
use crate::icons;
use crate::load::DocState;
use crate::theme;
use crate::ui::editor;
use crate::ui::titlebar::chord;

/// Recent entries shown in the Open menu. The state keeps more.
const RECENT_SHOWN: usize = 8;
const MENU_MIN_W: f32 = 240.0;
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

/// The Open File dialog, shared by the Open menu and ⌘O.
pub fn open_file_dialog(tx: &mut Tx) {
    let options = FileDialogOptions { title: "Open Markdown".into(), ..Default::default() }.filter("Markdown", MARKDOWN_EXTS);
    tx.open_file(options, |s: &mut AppState, r, _: &mut Tx| {
        let now = s.now;
        s.dialog_answer(r, now);
    });
}

/// New Markdown File: an untitled tab, switched to Edit with its source focused. Shared by the
/// Open menu and ⌘N.
pub(super) fn new_file(s: &mut AppState) {
    s.menu = None;
    s.new_untitled();
    editor::enter(s);
}

/// The Open Folder dialog, shared by the Open menu and ⇧⌘O.
pub fn open_folder_dialog(tx: &mut Tx) {
    let options = FileDialogOptions { title: "Open Folder".into(), ..Default::default() };
    tx.open_folder(options, |s: &mut AppState, r, _: &mut Tx| {
        let now = s.now;
        s.dialog_answer(r, now);
    });
}

pub(super) fn open_file(s: &mut AppState, tx: &mut Tx) {
    s.menu = None;
    open_file_dialog(tx);
}

pub(super) fn open_folder(s: &mut AppState, tx: &mut Tx) {
    s.menu = None;
    open_folder_dialog(tx);
}

/// Opens a Recent entry. One that no longer exists is dropped from the list instead.
pub(super) fn pick_recent(s: &mut AppState, path: &Path) {
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
                .shortcut(chord(false, "N"))
                .on_select(new_file),
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

/// Copies the active tab's path. Shared by the More menu and ⇧⌘C.
pub(super) fn copy_path(s: &mut AppState, tx: &mut Tx) {
    s.menu = None;
    if let Some(tab) = s.tabs.active() {
        tx.copy(tab.path.display().to_string());
    }
}

/// The label for showing a file's folder in the system's file manager.
pub fn reveal_label() -> &'static str {
    if cfg!(target_os = "macos") {
        "Show in Finder"
    } else if cfg!(target_os = "windows") {
        "Show in Explorer"
    } else {
        "Show in Files"
    }
}

/// Opens the folder that holds `path` in the file manager. Rune's `open_url` refuses `file:`
/// links, so the platform's opener runs on the folder directly, on a thread of its own so the
/// handler returns at once.
pub(super) fn reveal_in_folder(path: &std::path::Path) {
    let Some(dir) = path.parent().map(std::path::Path::to_path_buf) else { return };
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "explorer"
    } else {
        "xdg-open"
    };
    std::thread::spawn(move || {
        let _ = std::process::Command::new(opener).arg(dir).status();
    });
}

/// Reloads the active tab from disk. Shared by the More menu and ⌘R.
pub(super) fn reload(s: &mut AppState) {
    s.menu = None;
    s.requeue_active_load();
}

/// Copies the active tab's Markdown source, as read from disk. Shared by the More menu.
pub(super) fn copy_source(s: &mut AppState, tx: &mut Tx) {
    s.menu = None;
    if let Some(DocState::Ready { src, .. }) = s.tabs.active().map(|tab| &tab.doc) {
        tx.copy(src.to_string());
    }
}

fn tab_path(s: &AppState, id: u64) -> Option<String> {
    s.tabs.iter().find(|t| t.id == id).map(|t| t.path.display().to_string())
}

/// The right-click menu of tab `id`, open at `point`. Its entry is pushed into `layers`.
pub fn tab_menu(s: &AppState, k: &Kit, id: u64, point: PointerPress, layers: &mut Vec<El>) {
    let t = theme::tokens(k.dark);
    let path = tab_path(s, id);
    let has_others = s.tabs.iter().count() > 1;
    let menu = context_menu("menu-tab", el(), Some(point))
        .min_width(MENU_MIN_W)
        .on_close(|s: &mut AppState| {
            s.menu = None;
            s.tab_menu_at = None;
        })
        .item(
            MenuItem::new("Close")
                .lead(lead("tabmenu-close", Some(&icons::X), t.fg2))
                .on_select(move |s: &mut AppState| {
                    s.menu = None;
                    s.request_close(id);
                }),
        )
        .item(
            MenuItem::new("Close Others")
                .lead(lead("tabmenu-others", None, t.fg2))
                .disabled(!has_others)
                .on_select(move |s: &mut AppState| {
                    s.menu = None;
                    s.close_clean(|i, at| i != at, id);
                }),
        )
        .item(
            MenuItem::new("Close to the Right")
                .lead(lead("tabmenu-right", None, t.fg2))
                .on_select(move |s: &mut AppState| {
                    s.menu = None;
                    s.close_clean(|i, at| i > at, id);
                }),
        )
        .separator()
        .item(
            MenuItem::new("Copy Path")
                .lead(lead("tabmenu-copy", Some(&icons::COPY), t.fg2))
                .disabled(path.is_none())
                .on_select_tx(move |s: &mut AppState, tx: &mut Tx| {
                    s.menu = None;
                    if let Some(path) = tab_path(s, id) {
                        tx.copy(path);
                    }
                }),
        )
        .item(
            MenuItem::new(reveal_label())
                .lead(lead("tabmenu-reveal", Some(&icons::FOLDER_OPEN), t.fg2))
                .disabled(path.is_none())
                .on_select_tx(move |s: &mut AppState, _: &mut Tx| {
                    s.menu = None;
                    if let Some(tab) = s.tabs.iter().find(|t| t.id == id) {
                        reveal_in_folder(&tab.path);
                    }
                }),
        );
    // The trigger is a placeholder: the menu opens at `point`, and nothing in the strip needs it back.
    menu.build(k, layers);
}

/// Closes the active tab, asking first when it has unsaved edits. Shared by the More menu and ⌘W.
pub(super) fn close_tab(s: &mut AppState) {
    s.menu = None;
    if let Some(id) = s.tabs.active().map(|tab| tab.id) {
        s.request_close(id);
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
    let has_source = matches!(s.tabs.active().map(|tab| &tab.doc), Some(DocState::Ready { .. }));
    Menu::new("menu-more")
        .trigger(trigger)
        .open(s.menu == Some(MenuKind::More))
        .min_width(MENU_MIN_W)
        .on_close(|s: &mut AppState| s.menu = None)
        .item(
            MenuItem::new("Reload from Disk")
                .lead(lead("menu-reload", None, t.fg2))
                .shortcut(chord(false, "R"))
                .disabled(!has_tab)
                .on_select(reload),
        )
        .item(
            MenuItem::new(reveal_label())
                .lead(lead("menu-reveal", Some(&icons::FOLDER_OPEN), t.fg2))
                .disabled(!has_tab)
                .on_select_tx(|s: &mut AppState, _: &mut Tx| {
                    s.menu = None;
                    if let Some(path) = s.tabs.active().map(|tab| tab.path.clone()) {
                        reveal_in_folder(&path);
                    }
                }),
        )
        .item(
            MenuItem::new("Copy Path")
                .lead(lead("menu-copy-path", Some(&icons::COPY), t.fg2))
                .shortcut(chord(true, "C"))
                .disabled(!has_tab)
                .on_select_tx(copy_path),
        )
        .item(
            MenuItem::new("Copy Markdown Source")
                .lead(lead("menu-copy-source", Some(&icons::FILE_TEXT), t.fg2))
                .disabled(!has_source)
                .on_select_tx(copy_source),
        )
        .separator()
        .item(
            MenuItem::new("Zoom In")
                .lead(lead("menu-zoom-in", Some(&icons::ZOOM_IN), t.fg2))
                .shortcut(chord(false, "+"))
                .on_select(|s: &mut AppState| {
                    s.zoom_in();
                    s.menu = None;
                }),
        )
        .item(
            MenuItem::new("Zoom Out")
                .lead(lead("menu-zoom-out", Some(&icons::ZOOM_OUT), t.fg2))
                .shortcut(chord(false, "−"))
                .on_select(|s: &mut AppState| {
                    s.zoom_out();
                    s.menu = None;
                }),
        )
        .item(
            MenuItem::new("Actual Size")
                .lead(lead("menu-zoom-reset", None, t.fg2))
                .shortcut(chord(false, "0"))
                .on_select(|s: &mut AppState| {
                    s.zoom_reset();
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
