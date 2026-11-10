//! The find bar (§4.8): a floating field with the match count, previous and next, and close,
//! pinned under the titlebar at the right edge.

use rune::{Color, El, TextStyle, Tx, icon, row, text};
use rune_kit::glyphs::{CHEVRON_DOWN, CHEVRON_UP};
use rune_kit::{IconButton, IconButtonVariant, Kit, TextInput};

use crate::app::AppState;
use crate::find::reveal_current;
use crate::icons;
use crate::theme::{self, Tokens};


/// The bar's height, its gap below the titlebar and its inset from the right edge (§4.8).
const BAR_H: f32 = 36.0;
const GAP: f32 = 8.0;
const INSET: f32 = 16.0;
/// The query field's width.
const FIELD_W: f32 = 200.0;

pub fn view(s: &AppState, k: &Kit, t: &Tokens) -> El {
    let has_matches = s.find.count() > 0;
    let field = TextInput::new(&s.find.query)
        .on_edit(|s: &mut AppState| &mut s.find.query)
        .on_change(|s: &mut AppState, _: &mut Tx| s.sync_find())
        .placeholder("Find")
        .plain()
        .width(Some(FIELD_W))
        .build(k)
        .key("find-input");
    let count = text(s.find.label()).font(TextStyle { size: 12.0, ..k.ty.small }).color(t.fg2).key("find-count");
    let prev = IconButton::new(&CHEVRON_UP).variant(IconButtonVariant::Ghost).help("Previous  Shift+Enter")
        .disabled(!has_matches)
        .on_press_tx(|s: &mut AppState, tx: &mut Tx| {
            s.find_step(false);
            reveal_current(s, tx);
        })
        .build(k)
        .key("find-prev");
    let next = IconButton::new(&CHEVRON_DOWN).variant(IconButtonVariant::Ghost).help("Next  Enter")
        .disabled(!has_matches)
        .on_press_tx(|s: &mut AppState, tx: &mut Tx| {
            s.find_step(true);
            reveal_current(s, tx);
        })
        .build(k)
        .key("find-next");
    let close = IconButton::new(&icons::X).variant(IconButtonVariant::Ghost).help("Close  Esc")
        .on_press(|s: &mut AppState| s.find.close())
        .build(k)
        .key("find-close");
    let bar = row().key("find-bar").h(BAR_H).px(8.0).gap(4.0).center_y().radius(8.0)
        .fill(k.c.layer_01).border(1.0, t.line).shadow(8.0, 24.0, Color::BLACK.alpha(0.14))
        .child(icon(&icons::SEARCH, theme::ICON).color(t.fg3))
        .child(field)
        .child(count)
        .child(prev)
        .child(next)
        .child(close);
    row().key("find-layer").pin(None, Some(theme::BAR_H + GAP), Some(INSET), None).child(bar)
}
