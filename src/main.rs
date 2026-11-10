//! Rune Markdown: a light Markdown viewer.

#![allow(dead_code)]

mod app;
mod icons;
mod load;
mod open;
mod paths;
mod session;
mod sidebar;
mod tabs;
mod theme;
mod ui;
mod worker;

use rune::AppSpec;

fn main() {
    rune::run(AppSpec::<app::AppState> { ..AppSpec::new("Markdown", (1100.0, 760.0), app::AppState::new, ui::view) });
}
