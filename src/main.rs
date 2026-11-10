//! Rune Markdown: a light Markdown viewer.

#![allow(dead_code)]

mod icons;
mod load;
mod paths;
mod sidebar;
mod tabs;
mod theme;

use rune::tok::{self, type_};
use rune::{AppSpec, Cx, El, col, text};

#[derive(Default)]
struct Hello;

fn view(_: &Hello, _: &mut Cx<Hello>) -> El {
    let t = tok::theme(false);
    col().w_full().h_full().center().fill(t.bg)
        .child(text("Markdown").font(type_::TITLE).color(t.text))
}

fn main() {
    rune::run(AppSpec::<Hello> { ..AppSpec::new("Markdown", (1100.0, 760.0), Hello::default, view) });
}
