//! The empty state (§4.9): shown when no tab is open. Open File and Open Folder, and a Recent card.

use rune::{El, Tx, col, el, icon, row, text, TextStyle};
use rune_kit::Kit;

use crate::app::AppState;
use crate::icons;
use crate::theme::{self, Tokens};
use crate::ui::menus::{open_file, open_folder, pick_recent};
use crate::ui::titlebar::chord;

/// Recent entries the empty state lists.
const RECENT_SHOWN: usize = 5;

/// A primary (accent) or secondary button, with its shortcut after the label.
fn button(key: &str, label: &str, shortcut: String, primary: bool, t: &Tokens, k: &Kit) -> El {
    let (fill, ink, edge) = if primary { (t.accent, rune::Color::WHITE, t.accent) } else { (k.c.page, t.fg, t.line_strong) };
    let mut b = row().key(key).h(32.0).px(12.0).gap(8.0).center_y().radius(6.0).fill(fill).border(1.0, edge)
        .child(text(label).font(k.ty.small_medium).color(ink));
    if !shortcut.is_empty() {
        b = b.child(text(shortcut).font(k.ty.small).color(ink).layer_opacity(0.7));
    }
    b
}

fn recent_card(s: &AppState, k: &Kit, t: &Tokens) -> El {
    let heading = TextStyle { size: 11.0, weight: 600.0, tracking: 0.44, ..k.ty.small };
    let mut card = col().key("empty-recent").w_full().mt(32.0).padding(4.0).radius(8.0).border(1.0, t.line)
        .child(row().h(28.0).px(8.0).center_y().child(text("RECENT").font(heading).color(t.fg3)));
    for (i, path) in s.recent.iter().take(RECENT_SHOWN).enumerate() {
        let is_dir = path.is_dir();
        let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
        let parent = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
        let target = path.clone();
        let glyph = if is_dir { &icons::FOLDER } else { &icons::FILE_TEXT };
        card = card.child(
            row().key(format!("empty-recent-{i}")).h(28.0).px(8.0).gap(8.0).center_y().radius(4.0)
                .on_press_tx(move |s: &mut AppState, _: &mut Tx| pick_recent(s, &target))
                .child(icon(glyph, 16.0).color(t.fg3))
                .child(text(name).font(k.ty.small_medium).color(t.fg).wrap_anywhere())
                .child(text(parent).font(k.ty.small).color(t.fg3)),
        );
    }
    card
}

/// The empty state, centred in the body. Its sidebar is hidden by the caller when there are no roots.
pub fn view(s: &AppState, dark: bool) -> El {
    let k = theme::kit(dark);
    let t = theme::tokens(dark);
    let badge = el().size(56.0, 56.0).radius(14.0).fill(t.accent_soft).center()
        .child(icon(&icons::FILE_TEXT, 26.0).color(t.accent));
    let title = TextStyle { size: 17.0, weight: 650.0, ..k.ty.body };
    let mut wrap = col().w_full().max_w(440.0).center().gap(4.0)
        .child(badge)
        .child(text("Open some Markdown").font(title).color(t.fg).mt(16.0))
        .child(text("Drop files or a folder anywhere on this window.").font(k.ty.body).color(t.fg2));
    let actions = row().mt(20.0).gap(8.0).justify_center()
        .child(button("empty-open-file", "Open File…", chord(false, "O"), true, t, k).on_press_tx(open_file))
        .child(button("empty-open-folder", "Open Folder…", chord(true, "O"), false, t, k).on_press_tx(open_folder));
    wrap = wrap.child(actions);
    if !s.recent.is_empty() {
        wrap = wrap.child(recent_card(s, k, t));
    }
    col().key("empty").w_full().grow(1.0).min_h(0.0).center().child(wrap)
}
