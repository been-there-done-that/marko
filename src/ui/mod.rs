//! The root view: the titlebar, a divider and the body, with the menus, toast and drop overlay
//! layered over them.

mod document;
mod empty;
pub mod keys;
mod menus;
mod sidebar_view;
mod titlebar;

use std::path::PathBuf;

use rune::{Cx, El, TextStyle, Tx, col, el, icon, row, text};
use rune_kit::Kit;

use crate::app::{AppState, Appearance};
use crate::icons;
use crate::theme::{self, Tokens};

/// Light or dark for this frame: the app's own setting, or the system's when it follows it.
fn is_dark(s: &AppState, system_dark: bool) -> bool {
    match s.appearance {
        Appearance::System => system_dark,
        Appearance::Light => false,
        Appearance::Dark => true,
    }
}

pub fn view(s: &AppState, cx: &mut Cx<AppState>) -> El {
    let dark = is_dark(s, cx.system_dark());
    let chrome = cx.chrome_insets();
    let k = theme::kit(dark);
    let t = theme::tokens(dark);
    let mut layers = Vec::new();
    let bar = titlebar::bar(s, k, dark, chrome, &mut layers);
    if s.toast.is_some() {
        layers.push(toast(s, k, t));
    }
    if s.drop_hover {
        layers.push(drop_overlay(k, t));
    }
    let base = col().key("shell").w_full().h_full().fill(k.c.page)
        .child(bar)
        .child(el().key("divider").w_full().h(1.0).fill(t.line))
        .child(body(s, dark, k));
    rune_kit::root(base, layers)
}

/// The sidebar (when shown) beside the document or the empty state. Dropped files land here.
fn body(s: &AppState, dark: bool, k: &Kit) -> El {
    let show_side = s.sidebar.visible && (!s.tabs.is_empty() || !s.sidebar.roots.is_empty());
    let main = if s.tabs.is_empty() { empty::view(s, dark) } else { document::view(s, k, dark) };
    let mut row_el = row().key("body").w_full().grow(1.0).min_h(0.0);
    if show_side {
        row_el = row_el.child(sidebar_view::view(s, dark)).child(el().w(1.0).h_full().fill(theme::tokens(dark).line));
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
