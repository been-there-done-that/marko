//! The root view: the titlebar, a divider and the body, with the menus layered over them.

pub mod keys;
mod menus;
mod titlebar;

use rune::{Cx, El, col, el};

use crate::app::{AppState, Appearance};
use crate::theme;

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
    let base = col().key("shell").w_full().h_full().fill(k.c.page)
        .child(bar)
        .child(el().key("divider").w_full().h(1.0).fill(t.line))
        .child(el().key("body").w_full().grow(1.0).min_h(0.0));
    rune_kit::root(base, layers)
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
