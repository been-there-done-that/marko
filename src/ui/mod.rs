//! The root view: the titlebar, a divider and the body, with the menus, toast and drop overlay
//! layered over them.

mod document;
pub(crate) mod editor;
mod empty;
mod find_bar;
pub mod keys;
pub(crate) mod menus;
mod palette;
mod sidebar_view;
pub(crate) mod tab_strip;
mod titlebar;

use std::path::PathBuf;

use rune::{Cx, El, TextStyle, Tx, col, el, icon, row, text};
use rune_kit::{Button, ButtonVariant, Dialog, Kit};

use crate::app::AppState;
use crate::icons;
use crate::theme::{Look, self, Tokens};

pub fn view(s: &AppState, cx: &mut Cx<AppState>) -> El {
    let look = theme::look(&s.settings, &s.user_themes, cx.system_dark());
    let chrome = cx.chrome_insets();
    let k = &look.kit;
    let t = &look.tokens;
    let mut layers = Vec::new();
    let bar = titlebar::bar(s, k, look, chrome, &mut layers);
    if s.toast.is_some() {
        layers.push(toast(s, k, t));
    }
    if s.find.open {
        layers.push(find_bar::view(s, k, t));
    }
    guard(s, k, &mut layers);
    palette::layers(s, k, t, &mut layers);
    if s.drop_hover {
        layers.push(drop_overlay(k, t));
    }
    let base = col().key("shell").w_full().h_full().fill(k.c.page)
        .child(bar)
        .child(el().key("divider").w_full().h(1.0).fill(t.line))
        .child(body(s, look, k));
    rune_kit::root(base, layers)
}

/// The sidebar (when shown) beside the document or the empty state. Dropped files land here.
fn body(s: &AppState, look: &'static Look, k: &Kit) -> El {
    let show_side = s.sidebar.visible && (!s.tabs.is_empty() || !s.sidebar.roots.is_empty());
    let main = if s.tabs.is_empty() {
        empty::view(s, look)
    } else if editor::shows(s) {
        editor::view(s, k, look)
    } else {
        document::view(s, k, look)
    };
    let main = match editor::conflict_banner(s, k, look) {
        Some(banner) => col().key("main").grow(1.0).min_h(0.0).child(banner).child(main),
        None => main,
    };
    let mut row_el = row().key("body").w_full().grow(1.0).min_h(0.0);
    if show_side {
        row_el = row_el.child(sidebar_view::view(s, look)).child(el().w(1.0).h_full().fill(look.tokens.line));
    }
    row_el
        .child(main)
        .on_drop_files(|s: &mut AppState, paths: &[PathBuf], _: &mut Tx| {
            let now = s.now;
            s.open_paths(paths, now);
        })
        .on_drop_hover(|s: &mut AppState, on: bool, _: &mut Tx| s.drop_hover = on)
}

/// The toast pill, top centre, 12pt under the titlebar (§4.11).
fn toast(s: &AppState, k: &Kit, t: &Tokens) -> El {
    let msg = s.toast.as_ref().map(|t| t.text.clone()).unwrap_or_default();
    let font = TextStyle { size: 12.0, weight: 500.0, ..k.ty.small };
    let pill = row().key("toast").h(30.0).px(12.0).gap(8.0).center_y().radius(15.0).fill(t.fg)
        .child(text(msg).font(font).color(k.c.page));
    row().key("toast-layer").pin(Some(0.0), Some(theme::BAR_H + 12.0), Some(0.0), None).justify_center().child(pill)
}

/// The unsaved-changes guard: asked before a tab with unsaved edits closes. Save writes the tab
/// (asking for a name first when it is untitled) and then closes it; Don't Save closes it as it
/// is; Cancel and the close button keep it open.
fn guard(s: &AppState, k: &Kit, layers: &mut Vec<El>) {
    let Some(tab) = s.guard.and_then(|id| s.tabs.iter().find(|t| t.id == id)) else { return };
    let name = tab.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let actions = row().w_full().padding(16.0).gap(8.0).justify_end()
        .child(Button::new("Cancel").variant(ButtonVariant::Ghost)
            .on_press(|s: &mut AppState| s.guard = None).build(k).key("guard-cancel"))
        .child(Button::new("Don't Save")
            .on_press(|s: &mut AppState| {
                if let Some(id) = s.guard.take() {
                    s.close_now(id);
                }
            }).build(k).key("guard-discard"))
        .child(Button::new("Save").variant(ButtonVariant::Submit)
            .on_press_tx(|s: &mut AppState, tx: &mut Tx| {
                if let Some(id) = s.guard.take() {
                    keys::save_tab(s, tx, id, false, true);
                }
            }).build(k).key("guard-save"));
    Dialog::new("guard", format!("Save changes to {name}?"))
        .width(400.0)
        .fit()
        .open(true)
        .on_close(|s: &mut AppState| s.guard = None)
        .child(actions)
        .build(k, layers);
}

/// The drop hover (§4.10): a scrim, a dashed inset and a card naming what a drop does.
fn drop_overlay(k: &Kit, t: &Tokens) -> El {
    let inset = el().pin(Some(12.0), Some(12.0), Some(12.0), Some(12.0)).radius(12.0).border(2.0, t.accent).border_dash(8.0, 6.0);
    let badge = el().size(44.0, 44.0).radius(12.0).fill(t.accent_soft).center()
        .child(icon(&icons::DOWNLOAD, 22.0).color(t.accent));
    let card = col().key("drop-card").center().gap(4.0).px(24.0).py(20.0).radius(12.0).fill(k.c.layer_01).border(1.0, t.line)
        .child(badge)
        .child(text("Drop to open").font(TextStyle { size: 15.0, weight: 650.0, ..k.ty.body }).color(t.fg))
        .child(text("Files open as tabs · folders join the sidebar").font(k.ty.small).color(t.fg2));
    col().key("drop-overlay").pin(Some(0.0), Some(0.0), Some(0.0), Some(0.0)).fill(k.c.page.alpha(0.7)).blur(3.0).center()
        .child(inset)
        .child(card)
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
