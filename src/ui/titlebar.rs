//! The titlebar (§4.2): sidebar toggle and Open on the left, the tab strip, Preview | Edit,
//! Find and More on the right.

use rune::{ChromeInsets, El, el, icon, row, text};
use rune_kit::shell::{TabItem, TabOrientation, TabReorder, TabStrip, WindowTitlebar};
use rune_kit::{IconButton, IconButtonVariant, Kit};

use crate::app::{AppState, MenuKind};
use crate::icons;
use crate::theme::{self, Tokens};
use crate::ui::menus;

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

/// The Preview | Edit segments. Edit is inert in phase 1: it has no press handler.
fn mode(k: &Kit, t: &Tokens, preview_active: bool) -> El {
    let seg = |key: &str, label: &str, i: &'static rune::Icon, active: bool, live: bool| {
        let ink = if active { t.fg } else { t.fg2 };
        let mut e = row().key(key).h(22.0).px(8.0).gap(4.0).center_y().radius(4.0)
            .child(icon(i, 14.0).color(if active { t.accent } else { ink }))
            .child(text(label).font(k.ty.small_medium).color(ink));
        if active {
            e = e.fill(k.c.page).border(1.0, t.line);
        }
        if !live {
            e = e.layer_opacity(0.5).help("Edit arrives in phase 2");
        }
        e
    };
    row().key("mode").h(26.0).padding(1.0).gap(1.0).center_y().radius(6.0).border(1.0, t.line)
        .child(seg("mode-preview", "Preview", &icons::EYE, preview_active, true))
        .child(seg("mode-edit", "Edit", &icons::PENCIL, false, false))
}

/// The titlebar band for this frame. Menus it opens are pushed into `layers`.
pub fn bar(s: &AppState, k: &Kit, dark: bool, chrome: ChromeInsets, layers: &mut Vec<El>) -> El {
    let t = theme::tokens(dark);

    let sidebar = ghost_button("Sidebar", format!("Toggle sidebar  {}", chord(false, "B")), &icons::PANEL_LEFT)
        .on_press(|s: &mut AppState| s.sidebar.visible = !s.sidebar.visible)
        .build(k)
        .key("tb-sidebar");
    let open_btn = ghost_button("Open", format!("Open  {}", chord(false, "O")), &icons::FOLDER_OPEN)
        .on_press(|s: &mut AppState| toggle_menu(s, MenuKind::Open))
        .build(k)
        .key("tb-open");
    let open = menus::open_menu(s, k, open_btn, layers);

    let ids: Vec<String> = s.tabs.iter().map(|tab| tab.id.to_string()).collect();
    let active = s.tabs.active().map(|tab| tab.id.to_string());
    let items = ids
        .iter()
        .zip(s.tabs.titles())
        .map(|(id, title)| TabItem::new(id.clone(), title).icon(&icons::FILE_TEXT));
    let strip = TabStrip::new("tabs", TabOrientation::Horizontal)
        .tabs(items)
        .active(active.as_deref())
        .drag_state(&s.drag, |s: &mut AppState| &mut s.drag)
        .on_select(|s: &mut AppState, id: &str, _: &mut rune::Tx| {
            if let Ok(id) = id.parse() {
                s.tabs.activate(id);
                s.sync_find();
            }
        })
        .on_close(|s: &mut AppState, id: &str, _: &mut rune::Tx| {
            if let Ok(id) = id.parse() {
                s.tabs.close(id);
                s.sync_find();
            }
        })
        .on_reorder(|s: &mut AppState, r: TabReorder, _: &mut rune::Tx| {
            if let Ok(id) = r.id.parse() {
                s.tabs.move_tab(id, r.to);
                s.sync_find();
            }
        })
        .build_layered(k, layers);

    let find = ghost_button("Find", format!("Find {}", chord(false, "F")), &icons::SEARCH)
        .on_press_tx(|s: &mut AppState, tx: &mut rune::Tx| crate::find::toggle(s, tx))
        .build(k)
        .key("tb-find");
    let more = ghost_button("More", "More", &icons::ELLIPSIS)
        .on_press(|s: &mut AppState| toggle_menu(s, MenuKind::More))
        .build(k)
        .key("tb-more");
    let more = menus::more_menu(s, k, more, layers);

    WindowTitlebar::from_insets(chrome, 1.0)
        .height(theme::BAR_H)
        .gap(4.0)
        .leading(sidebar)
        .leading(open)
        .leading(divider(t))
        .tabs(strip)
        .trailing(mode(k, t, true))
        .trailing(divider(t))
        .trailing(find)
        .trailing(more)
        .build(k)
}
