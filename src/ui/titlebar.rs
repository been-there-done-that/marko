//! The titlebar (§4.2): sidebar toggle and Open on the left, the tab strip, Preview | Edit,
//! Find and More on the right.

use rune::{ChromeInsets, El, PointerPress, TextStyle, Tx, el, icon, row, text};
use rune_kit::shell::{TabItem, TabItemStatus, TabOrientation, TabOverflow, TabReorder, TabStrip, WindowTitlebar};
use rune_kit::{IconButton, IconButtonVariant, Kit};

use crate::app::{AppState, Mode, MenuKind};
use crate::icons;
use crate::stats;
use crate::theme::{self, Tokens};
use crate::ui::{editor, menus};

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
    text(stats::text(s.stats)).key("stats").font(font).color(t.fg3)
}

/// The narrowest a tab may be made to fit its name, and the widest.
const TAB_MIN_W: f32 = 96.0;
const TAB_MAX_W: f32 = 240.0;
/// What a tab adds around its title: padding and avatar, the gap, the title's end inset when
/// emphasized, and the status or close slot (`tab_strip/tab.rs`: 6 + 16 + 6 + 24 + 20 + 6).
const TAB_CHROME_W: f32 = 78.0;

/// The width a tab needs for `title` in the tab font, as rune measures it.
fn tab_content_width(title: &str, k: &Kit) -> f32 {
    // The tab strip sets its titles at this weight and size (`tab/tab.rs` `title`).
    let font = TextStyle { weight: k.ty.ui_medium.weight, ..k.ty.ui_compact };
    rune::text::measure(title, &[], font, None).0 + TAB_CHROME_W
}

/// The tab width cap: the longest title's content width, clamped to [`TAB_MIN_W`, `TAB_MAX_W`].
/// Tabs share the strip equally up to this, so short names make narrow tabs.
pub fn tab_max_width<'a>(titles: impl IntoIterator<Item = &'a str>, k: &Kit) -> f32 {
    let longest = titles.into_iter().map(|t| tab_content_width(t, k)).fold(0.0, f32::max);
    longest.clamp(TAB_MIN_W, TAB_MAX_W)
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

    let active_id = s.tabs.active().map(|tab| tab.id);
    let active = active_id.map(|id| id.to_string());
    // A background tab changed on disk, or with unsaved edits, shows an accent dot. A changed
    // background tab's dot clears when it is activated; a dirty tab keeps its dot until saved.
    let titles = s.tabs.titles();
    let max_w = tab_max_width(titles.iter().map(String::as_str), k);
    let items = s.tabs.iter().zip(titles.iter()).map(|(tab, title)| {
        let unread = (tab.changed && Some(tab.id) != active_id) || editor::dirty(tab);
        let (id, path) = (tab.id, tab.path.display().to_string());
        TabItem::new(tab.id.to_string(), title).icon(&icons::FILE_TEXT).status(unread.then_some(TabItemStatus::Unread)).decorate(move |slot, _| {
            slot.help(path).on_secondary_press(move |s: &mut AppState, p: PointerPress, _: &mut Tx| {
                s.tab_menu_at = Some(p);
                s.menu = Some(MenuKind::TabContext { id });
            })
        })
    });
    let strip = TabStrip::new("tabs", TabOrientation::Horizontal)
        .overflow(TabOverflow::ShrinkThenScroll { min_width: 28.0, max_width: max_w, pinned_width: 36.0, fade_width: 24.0 })
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
                s.request_close(id);
            }
        })
        .on_reorder(|s: &mut AppState, r: TabReorder, _: &mut rune::Tx| {
            if let Ok(id) = r.id.parse() {
                s.tabs.move_tab(id, r.to);
                s.sync_find();
            }
        })
        .build_layered(k, layers);
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

    let mut bar = WindowTitlebar::from_insets(chrome, 1.0)
        .height(theme::BAR_H)
        .gap(4.0)
        .leading(sidebar)
        .leading(open)
        .leading(divider(t))
        .tabs(strip);
    if s.stats_on && s.stats_last.is_some() {
        bar = bar.trailing(readout(s, k, t));
    }
    bar.trailing(mode(s, k, t))
        .trailing(divider(t))
        .trailing(find)
        .trailing(more)
        .build(k)
}

#[cfg(test)]
#[path = "titlebar_tests.rs"]
mod tests;
