//! The titlebar's tab strip (§4.2): one tab per open document, each as wide as its own name
//! asks (between a minimum and a cap), in a row that scrolls sideways when they overflow.
//!
//! The app draws the strip itself. rune-kit's `TabStrip` gives every tab one shared width, and
//! its drag reorder assumes that width too, so variable widths and drag don't mix there. Tabs
//! move with Ctrl+Shift+PageUp and PageDown instead (`ui/keys.rs`).

use rune::{El, PointerButton, PointerPress, ScrollFade, TextStyle, Tx, el, icon, row, text};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Kit};

use crate::app::{AppState, MenuKind};
use crate::icons;
use crate::tabs::Tab;
use crate::theme::Tokens;
use crate::ui::editor;

/// The key of the strip's scroll viewport, which the active tab is revealed in.
const SCROLL: &str = "tabs-scroll";
/// A tab's height, corner radius and the space between tabs.
const HEIGHT: f32 = 28.0;
const RADIUS: f32 = 6.0;
const TAB_GAP: f32 = 4.0;
/// Horizontal padding inside a tab, and the gap between its icon, title and close slot.
const PAD: f32 = 12.0;
const GAP: f32 = 6.0;
/// The file icon, the status and close slot, and the accent dot in that slot.
const ICON: f32 = 14.0;
const SLOT: f32 = 20.0;
const DOT: f32 = 6.0;
/// The narrowest and widest a tab is.
const MIN_W: f32 = 72.0;
const MAX_W: f32 = 240.0;
/// The edge fade where the strip scrolls sideways.
const FADE: f32 = 24.0;
/// What a tab adds around its title: both paddings, the icon, both gaps and the slot.
const CHROME_W: f32 = PAD * 2.0 + ICON + GAP * 2.0 + SLOT;

/// The title's font: the kit's compact UI size at medium weight.
fn title_font(k: &Kit) -> TextStyle {
    TextStyle { weight: k.ty.ui_medium.weight, ..k.ty.ui_compact }
}

/// The width a tab takes for `title`: its text plus the chrome, kept between [`MIN_W`] and
/// [`MAX_W`]. A title wider than that is cut in the middle.
pub fn tab_width(title: &str, k: &Kit) -> f32 {
    (rune::text::measure(title, &[], title_font(k), None).0 + CHROME_W).clamp(MIN_W, MAX_W)
}

/// The strip for the titlebar's middle: the tabs in a scrolling row that fills the space given.
pub fn view(s: &AppState, k: &Kit, t: &Tokens) -> El {
    let active = s.tabs.active().map(|tab| tab.id);
    let titles = s.tabs.titles();
    let tabs: Vec<El> = s.tabs.iter().zip(titles.iter()).map(|(tab, title)| tab_el(tab, title, active == Some(tab.id), k, t)).collect();
    let list = row().key("tab-list").role("tablist").label("Tabs").h(HEIGHT).gap(TAB_GAP).center_y().children(tabs);
    let scroll = row()
        .key(SCROLL)
        .h(HEIGHT)
        .shrink()
        .min_w(0.0)
        .scroll_x()
        .scroll_fade(ScrollFade::strength(FADE).linear().full_at(0.1))
        .scroll_margin(FADE)
        .child(list);
    row().key("tabs").grow(1.0).shrink().min_w(0.0).center_y().child(scroll)
}

/// One tab: its icon, its title cut in the middle, and the slot that holds the close button,
/// or the accent dot while the tab has changes the pointer hasn't reached yet.
fn tab_el(tab: &Tab, title: &str, active: bool, k: &Kit, t: &Tokens) -> El {
    let id = tab.id;
    let unread = (tab.changed && !active) || editor::dirty(tab);
    let ink = if active { t.fg } else { t.fg2 };
    let mark = if active { t.accent } else { t.fg3 };

    // The dot shows at rest and gives way to the close button under the pointer. The close
    // button shows under the pointer, and at rest on the active tab when there's no dot.
    let dot = el()
        .key("status-mark")
        .absolute((SLOT - DOT) / 2.0, (SLOT - DOT) / 2.0)
        .size(DOT, DOT)
        .radius(DOT / 2.0)
        .fill(k.c.bg_accent)
        .group_opacity(if unread { 1.0 } else { 0.0 }, 0.0)
        .instant_changes();
    let button = IconButton::new(k.glyph.close)
        .control_size(ControlSize::Small)
        .variant(IconButtonVariant::GhostMuted)
        .label("Close tab")
        .on_press_tx(move |s: &mut AppState, _: &mut Tx| s.request_close(id))
        .build(k)
        .key("close");
    let close = row()
        .absolute(0.0, 0.0)
        .size(SLOT, SLOT)
        .center()
        .group_opacity(if active && !unread { 1.0 } else { 0.0 }, 1.0)
        .instant_changes()
        .child(button);
    let slot = el().size(SLOT, SLOT).child(dot).child(close);

    let label = text(title.to_owned()).key("label").font(title_font(k)).color(ink).grow(1.0).shrink().truncate_middle().a11y_hidden();
    let pill = row()
        .key(format!("t-{id}"))
        .role("tab")
        .label(title.to_owned())
        .help(tab.path.display().to_string())
        .w(tab_width(title, k))
        .h(HEIGHT)
        .radius(RADIUS)
        .px(PAD)
        .gap(GAP)
        .center_y()
        .hover_group_focus()
        .focus_selected(active)
        .pointer_buttons([PointerButton::Middle])
        .on_press_tx(move |s: &mut AppState, tx: &mut Tx| {
            if tx.button() == PointerButton::Middle {
                s.request_close(id);
            } else {
                s.tabs.activate(id);
                s.sync_find();
            }
        })
        .on_secondary_press(move |s: &mut AppState, p: PointerPress, _: &mut Tx| {
            s.tab_menu_at = Some(p);
            s.menu = Some(MenuKind::TabContext { id });
        })
        .child(icon(&icons::FILE_TEXT, ICON).color(mark))
        .child(label)
        .child(slot);
    if active {
        pill.fill(k.c.page).border(1.0, t.line)
    } else {
        pill.hover(t.hover)
    }
}

/// Scrolls the strip to the active tab whenever the active tab changes, so one that opens,
/// cycles or moves stays in view. Called once per frame from `poll`.
pub fn reveal_active(s: &mut AppState, tx: &mut Tx) {
    let active = s.tabs.active().map(|t| t.id);
    if active == s.tab_revealed {
        return;
    }
    s.tab_revealed = active;
    if let Some(id) = active {
        tx.scroll_into_view_in(SCROLL, format!("t-{id}"));
    }
}
