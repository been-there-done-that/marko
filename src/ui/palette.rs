//! The command palette: a field and the best matches in a panel under the titlebar.

use rune::{Color, El, TextStyle, Tx, col, el, icon, row, text};
use rune_kit::{Kit, TextInput};

use crate::app::AppState;
use crate::commands::{self, Command};
use crate::icons;
use crate::theme::{self, Tokens};

/// The panel's width, and its gap below the titlebar.
const WIDTH: f32 = 560.0;
const GAP: f32 = 48.0;
const ROW_H: f32 = 32.0;

/// Pushes the scrim and the panel while the palette is open.
pub fn layers(s: &AppState, k: &Kit, t: &Tokens, layers: &mut Vec<El>) {
    if !s.palette.open {
        return;
    }
    layers.push(el().key("palette-scrim").absolute(0.0, 0.0).w_full().h_full().fill(k.c.scrim).a11y_hidden());
    layers.push(
        row().key("palette-layer").absolute(0.0, 0.0).w_full().h_full().pt(theme::BAR_H + GAP).px(16.0)
            .justify_center().items_start().child(panel(s, k, t)),
    );
}

fn panel(s: &AppState, k: &Kit, t: &Tokens) -> El {
    let rows = commands::ranked(s, &s.palette.query.text);
    let field = TextInput::new(&s.palette.query)
        .on_edit(|s: &mut AppState| &mut s.palette.query)
        .on_change(|s: &mut AppState, _: &mut Tx| s.palette.selected = 0)
        .on_key(crate::ui::keys::palette_field_key)
        .placeholder("Type a command")
        .plain()
        .build(k)
        .key("palette-input");
    let head = row().w_full().h(44.0).px(12.0).gap(8.0).center_y()
        .child(icon(&icons::SEARCH, theme::ICON).color(t.fg3))
        .child(field.grow(1.0));
    let mut list = col().key("palette-list").w_full().padding(6.0).gap(2.0);
    if rows.is_empty() {
        list = list.child(el().h(ROW_H).px(8.0).center_y().child(text("No matching commands").font(k.ty.small).color(t.fg3)));
    }
    for (i, c) in rows.iter().enumerate() {
        list = list.child(command_row(i, c, i == s.palette.selected, k, t));
    }
    col().key("palette").w_full().max_w(WIDTH).radius(10.0).fill(k.c.layer_01).border(1.0, t.line)
        .shadow(12.0, 32.0, Color::BLACK.alpha(0.2))
        .child(head)
        .child(el().w_full().h(1.0).fill(t.line))
        .child(list)
        .modal(true, |s: &mut AppState| s.palette.close())
}

fn command_row(i: usize, c: &Command, selected: bool, k: &Kit, t: &Tokens) -> El {
    let id = c.id.clone();
    let mut r = row().key(format!("palette-row-{i}")).w_full().h(ROW_H).px(8.0).gap(8.0).center_y().radius(6.0)
        .hover(t.accent_soft.alpha(0.5))
        .on_press_tx(move |s: &mut AppState, tx: &mut Tx| commands::run(s, &id, tx))
        .child(text(c.label.clone()).font(k.ty.body).color(t.fg).grow(1.0));
    if selected {
        r = r.fill(t.accent_soft);
    }
    if let Some(h) = &c.hint {
        r = r.child(text(h.clone()).font(TextStyle { size: 12.0, ..k.ty.small }).color(t.fg3));
    }
    r
}

#[cfg(test)]
#[path = "palette_tests.rs"]
mod tests;
