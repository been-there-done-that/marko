//! The titlebar (§4.2): sidebar toggle and Open on the left, the tab strip, Preview | Edit,
//! Find and More on the right.

use rune::{ChromeInsets, El, TextStyle, el, icon, row, text};
use rune_kit::shell::WindowTitlebar;
use rune_kit::{IconButton, IconButtonVariant, Kit};

use crate::app::{AppState, Mode, MenuKind};
use crate::icons;
use crate::stats;
use crate::theme::{Look, self, Tokens};
use crate::ui::{editor, menus, tab_strip};

/// Shortcut text for a key on the current platform: `⌘O` on macOS, `Ctrl+O` elsewhere.
pub fn chord(shift: bool, key: &str) -> String {
    match (cfg!(target_os = "macos"), shift) {
        (true, false) => format!("⌘{key}"),
        (true, true) => format!("⌘⇧{key}"),
        (false, false) => format!("Ctrl+{key}"),
        (false, true) => format!("Ctrl+Shift+{key}"),
    }
}

/// Opens `kind`'s menu, or closes it when it is already open.
pub fn toggle_menu(s: &mut AppState, kind: MenuKind) {
    s.menu = if s.menu == Some(kind) { None } else { Some(kind) };
}

fn ghost_button(label: &str, help: impl Into<String>, i: &'static rune::Icon) -> IconButton {
    IconButton::new(i).label(label).help(help).variant(IconButtonVariant::Ghost)
}

fn divider(t: &Tokens) -> El {
    el().w(1.0).h(16.0).mx(4.0).fill(t.line_strong)
}

/// The Preview | Edit segments. Each switches the document's mode.
fn mode(s: &AppState, k: &Kit, t: &Tokens) -> El {
    let editing = matches!(s.mode, Mode::Edit);
    let seg = |key: &str, label: &str, i: &'static rune::Icon, active: bool| {
        let ink = if active { t.fg } else { t.fg2 };
        let mut e = row().key(key).h(22.0).px(8.0).gap(4.0).center_y().radius(4.0)
            .child(icon(i, 14.0).color(if active { t.accent } else { ink }))
            .child(text(label).font(k.ty.small_medium).color(ink));
        if active {
            e = e.fill(k.c.page).border(1.0, t.line);
        }
        e
    };
    row().key("mode").h(26.0).padding(1.0).gap(1.0).center_y().radius(6.0).border(1.0, t.line)
        .child(seg("mode-preview", "Preview", &icons::EYE, !editing).on_press(|s: &mut AppState| editor::preview(s)))
        .child(seg("mode-edit", "Edit", &icons::PENCIL, editing).on_press(|s: &mut AppState| {
            editor::enter(s);
        }))
}

/// This process's memory and CPU as plain numbers, in small text left of the mode switch.
fn readout(s: &AppState, k: &Kit, t: &Tokens) -> El {
    let font = TextStyle { size: 12.0, features: "'tnum' 1", ..k.ty.small };
    text(stats::text(s.stats)).key("stats").font(font).color(t.fg3).live_text(&s.stats_text)
}

/// The sidebar toggle and the Open button (with its menu), shared by both layouts.
fn side_controls(s: &AppState, k: &Kit, layers: &mut Vec<El>) -> (El, El) {
    let sidebar = ghost_button("Sidebar", format!("Toggle sidebar  {}", chord(false, "B")), &icons::PANEL_LEFT)
        .on_press_tx(|s: &mut AppState, tx: &mut rune::Tx| s.toggle_sidebar(tx))
        .build(k)
        .key("tb-sidebar");
    let open_btn = ghost_button("Open", format!("Open  {}", chord(false, "O")), &icons::FOLDER_OPEN)
        .on_press(|s: &mut AppState| toggle_menu(s, MenuKind::Open))
        .build(k)
        .key("tb-open");
    (sidebar, menus::open_menu(s, k, open_btn, layers))
}

/// The header at the top of the full-height sidebar: the sidebar toggle and Open at the left,
/// clear of the macOS traffic lights, in exactly the place the toolbar puts them when the sidebar
/// is hidden (same inset, padding and gap), so toggling never moves them; the app's name at the
/// right. It is a window drag region like the toolbar beside it.
pub fn side_header(s: &AppState, k: &Kit, look: &'static Look, chrome: ChromeInsets, layers: &mut Vec<El>) -> El {
    let t = &look.tokens;
    let (sidebar, open) = side_controls(s, k, layers);
    let name = text("Marko").font(k.ty.small_medium).color(t.fg);
    // `WindowTitlebar`'s own leading padding and this bar's gap (see `bar`).
    row().key("side-header").w_full().h(theme::BAR_H).pl(chrome.leading + 14.0).pr(14.0).gap(4.0).center_y()
        .window_drag()
        .child(sidebar)
        .child(open)
        .child(row().grow(1.0).min_w(0.0).justify_end().center_y().child(name))
}

/// The toolbar band for this frame. Menus it opens are pushed into `layers`. With the sidebar
/// shown (`side`) the band spans only the document column: the traffic lights are in the
/// sidebar's header, so it reserves no leading inset and carries no sidebar controls.
pub fn bar(s: &AppState, k: &Kit, look: &'static Look, chrome: ChromeInsets, side: bool, layers: &mut Vec<El>) -> El {
    let t = &look.tokens;
    let strip = tab_strip::view(s, k, t);
    if let (Some(MenuKind::TabContext { id }), Some(point)) = (s.menu, s.tab_menu_at) {
        menus::tab_menu(s, k, id, point, layers);
    }

    let find = ghost_button("Find", format!("Find {}", chord(false, "F")), &icons::SEARCH)
        .on_press_tx(|s: &mut AppState, tx: &mut rune::Tx| crate::find::toggle(s, tx))
        .build(k)
        .key("tb-find");
    let more = ghost_button("More", "More", &icons::ELLIPSIS)
        .on_press(|s: &mut AppState| toggle_menu(s, MenuKind::More))
        .build(k)
        .key("tb-more");
    let more = menus::more_menu(s, k, more, layers);

    let mut insets = chrome;
    if side {
        insets.leading = 0.0;
    }
    let mut bar = WindowTitlebar::from_insets(insets, 1.0).height(theme::BAR_H).gap(4.0);
    if !side {
        let (sidebar, open) = side_controls(s, k, layers);
        bar = bar.leading(sidebar).leading(open).leading(divider(t));
    }
    let mut bar = bar.tabs(strip);
    if s.stats_on && s.stats_last.is_some() {
        bar = bar.trailing(readout(s, k, t));
    }
    if s.has_background() {
        // The default band is opaque; over a picture it is the theme's translucent bar colour.
        bar = bar.fill(t.bar);
    }
    bar.trailing(mode(s, k, t))
        .trailing(divider(t))
        .trailing(find)
        .trailing(more)
        .build(k)
        .backdrop_blur(super::frost(s))
}

#[cfg(test)]
#[path = "titlebar_tests.rs"]
mod tests;
