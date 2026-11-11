//! The command palette's commands: what it lists, how a typed query ranks them, and what each
//! one does.

use rune::{TextEdit, Tx};

use crate::app::AppState;
use crate::find::toggle as toggle_find;
use crate::themes;
use crate::ui::{editor, keys, menus};

/// Most rows the palette shows.
pub const MAX_ROWS: usize = 12;

/// One entry of the palette. `id` is what `run` takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Command {
    pub id: String,
    pub label: String,
    /// A shortcut or other note shown at the row's right edge.
    pub hint: Option<String>,
}

/// The palette's state: whether it is up, the query and the highlighted row.
pub struct Palette {
    pub open: bool,
    pub query: TextEdit,
    pub selected: usize,
}

impl Palette {
    pub fn new() -> Self {
        Self { open: false, query: TextEdit::new(""), selected: 0 }
    }

    /// Shows the palette with an empty query and the field focused.
    pub fn show(&mut self) {
        self.open = true;
        self.query.replace_text("");
        self.query.focused = true;
        self.selected = 0;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.query.focused = false;
    }
}

fn cmd(id: &str, label: &str, hint: Option<&str>) -> Command {
    Command { id: id.into(), label: label.into(), hint: hint.map(Into::into) }
}

/// The primary modifier as the shortcut hints spell it.
fn primary() -> &'static str {
    if cfg!(target_os = "macos") { "⌘" } else { "Ctrl+" }
}

/// Every command, in the order the empty query lists them.
pub fn all(s: &AppState) -> Vec<Command> {
    let p = primary();
    let k = |key: &str| format!("{p}{key}");
    let mut out = vec![
        cmd("open-file", "Open File", Some(&k("O"))),
        cmd("open-folder", "Open Folder", Some(&k("Shift+O"))),
        cmd("new-file", "New File", Some(&k("N"))),
        cmd("save", "Save", Some(&k("S"))),
        cmd("toggle-sidebar", "Toggle Sidebar", Some(&k("B"))),
        cmd("find", "Find", Some(&k("F"))),
        cmd("toggle-edit", "Preview / Edit", Some(&k("E"))),
        cmd("zoom-in", "Zoom In", Some(&k("+"))),
        cmd("zoom-out", "Zoom Out", Some(&k("-"))),
        cmd("zoom-reset", "Actual Size", Some(&k("0"))),
        cmd("wide", "Wide Reading Column", None),
        cmd("settings", "Settings", None),
    ];
    for def in themes::builtin().into_iter().chain(s.user_themes.iter().cloned()) {
        out.push(Command { id: format!("theme:{}", def.id), label: format!("Theme: {}", def.name), hint: None });
    }
    for path in &s.recent {
        let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string());
        out.push(Command { id: format!("recent:{}", path.display()), label: format!("Open Recent: {name}"), hint: None });
    }
    out
}

/// The commands matching `query`, best first, at most `MAX_ROWS`. An empty query lists them in
/// `all` order.
pub fn ranked(s: &AppState, query: &str) -> Vec<Command> {
    let mut scored: Vec<(i32, Command)> =
        all(s).into_iter().filter_map(|c| fuzzy(query, &c.label).map(|score| (score, c))).collect();
    // Stable, so equal scores keep their listed order.
    scored.sort_by_key(|(score, _)| std::cmp::Reverse(*score));
    scored.into_iter().take(MAX_ROWS).map(|(_, c)| c).collect()
}

/// A subsequence match of `query` in `label`, ignoring case. None when some letter of the query
/// has no place; otherwise a score where higher is better: hits at the start of a word and hits
/// next to the previous one count extra, and gaps and a late first hit cost a little.
pub fn fuzzy(query: &str, label: &str) -> Option<i32> {
    let label: Vec<char> = label.chars().flat_map(char::to_lowercase).collect();
    let mut score = 0;
    let mut at = 0;
    let mut prev: Option<usize> = None;
    for q in query.chars().filter(|c| !c.is_whitespace()).flat_map(char::to_lowercase) {
        let i = at + label[at..].iter().position(|&c| c == q)?;
        if i == 0 || !label[i - 1].is_alphanumeric() {
            score += 10;
        }
        match prev {
            Some(p) if i == p + 1 => score += 5,
            Some(p) => score -= (i - p - 1).min(5) as i32,
            None => score -= i.min(10) as i32,
        }
        prev = Some(i);
        at = i + 1;
    }
    Some(score)
}

/// Runs command `id`. The palette closes first, so a command that opens something owns the focus.
pub fn run(s: &mut AppState, id: &str, tx: &mut Tx) {
    s.palette.close();
    if let Some(theme) = id.strip_prefix("theme:") {
        s.settings.theme = theme.into();
        return;
    }
    if let Some(path) = id.strip_prefix("recent:") {
        let now = s.now;
        s.open_paths(&[path.into()], now);
        return;
    }
    match id {
        "open-file" => menus::open_file_dialog(tx),
        "open-folder" => menus::open_folder_dialog(tx),
        "new-file" => {
            s.new_untitled();
            editor::enter(s);
        }
        "save" => {
            if let Some(tab) = s.tabs.active().map(|t| t.id) {
                keys::save_tab(s, tx, tab, false, false);
            }
        }
        "toggle-sidebar" => s.toggle_sidebar(tx),
        "find" => {
            if !s.find.open {
                toggle_find(s, tx);
            } else {
                s.find.query.focused = true;
            }
        }
        "toggle-edit" => {
            editor::toggle(s);
        }
        "zoom-in" => s.zoom_in(),
        "zoom-out" => s.zoom_out(),
        "zoom-reset" => s.zoom_reset(),
        "wide" => s.wide = !s.wide,
        "settings" => s.settings_open = true,
        _ => {}
    }
}

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
