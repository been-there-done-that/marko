//! Theme definitions: the built-in set, and the reader's own JSON files in the config folder's
//! `themes/` (one theme per `*.json` file, the same fields as `ThemeDef`).

use std::path::Path;

use serde::{Deserialize, Serialize};

/// One theme: every colour the app and its code highlighting draw with, as `#rrggbb`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ThemeDef {
    /// Stable id saved in settings. A user theme's id is its file stem.
    #[serde(default)]
    pub id: String,
    pub name: String,
    pub dark: bool,
    /// The reading surface.
    pub bg: String,
    pub fg: String,
    pub fg2: String,
    pub fg3: String,
    pub accent: String,
    /// Titlebar.
    pub bar: String,
    /// Sidebar.
    pub side: String,
    pub line: String,
    pub line_strong: String,
    pub code_bg: String,
    pub code_line: String,
    pub syntax: SyntaxColors,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxColors {
    pub keyword: String,
    pub string: String,
    pub function: String,
    pub number: String,
    pub comment: String,
    #[serde(rename = "type")]
    pub ty: String,
}

/// Builds a `ThemeDef` from literals: id, name, dark, then the colours in field order.
macro_rules! theme {
    ($id:expr, $name:expr, $dark:expr, bg $bg:expr, fg $fg:expr, fg2 $fg2:expr, fg3 $fg3:expr, accent $accent:expr,
     bar $bar:expr, side $side:expr, line $line:expr, strong $strong:expr, code $code:expr, code_line $code_line:expr,
     syntax [$k:expr, $s:expr, $f:expr, $n:expr, $c:expr, $t:expr]) => {
        ThemeDef {
            id: $id.into(), name: $name.into(), dark: $dark,
            bg: $bg.into(), fg: $fg.into(), fg2: $fg2.into(), fg3: $fg3.into(), accent: $accent.into(),
            bar: $bar.into(), side: $side.into(), line: $line.into(), line_strong: $strong.into(),
            code_bg: $code.into(), code_line: $code_line.into(),
            syntax: SyntaxColors { keyword: $k.into(), string: $s.into(), function: $f.into(), number: $n.into(), comment: $c.into(), ty: $t.into() },
        }
    };
}

/// The themes that ship with the app, in menu order. `light` and `dark` are what `auto` follows.
pub fn builtin() -> Vec<ThemeDef> {
    vec![
        theme!("light", "Light", false, bg "#FFFFFF", fg "#1C1917", fg2 "#57534E", fg3 "#A8A29E", accent "#2F6BFF",
            bar "#F5F5F4", side "#FAFAF9", line "#E7E5E4", strong "#D6D3D1", code "#F7F7F6", code_line "#ECEBEA",
            syntax ["#B4237A", "#1A7F37", "#2F5FD0", "#A15C00", "#8C8781", "#7A3FC2"]),
        theme!("dark", "Dark", true, bg "#18181B", fg "#EDEDEF", fg2 "#A1A1AA", fg3 "#63636B", accent "#6B93FF",
            bar "#1F1F23", side "#1C1C20", line "#2C2C31", strong "#3A3A40", code "#1F1F23", code_line "#2C2C31",
            syntax ["#FF7AB8", "#7EE2A0", "#8FB2FF", "#F5B86B", "#73737D", "#C7A1FF"]),
        theme!("paper", "Paper", false, bg "#F6F0E1", fg "#3B2F2A", fg2 "#6B5B4E", fg3 "#A08E7C", accent "#B35C1E",
            bar "#EDE4CF", side "#F1E9D6", line "#E2D6BC", strong "#D2C2A2", code "#EFE6D2", code_line "#E2D6BC",
            syntax ["#A23B5A", "#4F7A28", "#2E6A9E", "#9A5B12", "#9C8B78", "#7B4BA0"]),
        theme!("solar-light", "Solar Light", false, bg "#FDF6E3", fg "#36474F", fg2 "#586E75", fg3 "#93A1A1", accent "#268BD2",
            bar "#F5EDD6", side "#F8F1DD", line "#EAE2CA", strong "#DCD3B8", code "#F5EDD6", code_line "#EAE2CA",
            syntax ["#859900", "#2AA198", "#268BD2", "#D33682", "#93A1A1", "#B58900"]),
        theme!("solar-dark", "Solar Dark", true, bg "#002B36", fg "#E4E4D6", fg2 "#93A1A1", fg3 "#586E75", accent "#2AA198",
            bar "#04313D", side "#012E3A", line "#0E3F4B", strong "#1A4F5C", code "#073642", code_line "#0E3F4B",
            syntax ["#859900", "#2AA198", "#268BD2", "#D33682", "#586E75", "#B58900"]),
        theme!("nordic", "Nordic", true, bg "#2E3440", fg "#ECEFF4", fg2 "#D8DEE9", fg3 "#7B88A1", accent "#88C0D0",
            bar "#323846", side "#2B303B", line "#3B4252", strong "#4C566A", code "#3B4252", code_line "#434C5E",
            syntax ["#81A1C1", "#A3BE8C", "#88C0D0", "#B48EAD", "#616E88", "#8FBCBB"]),
        theme!("midnight", "Midnight", true, bg "#0B0D12", fg "#E6E8EE", fg2 "#9AA1B2", fg3 "#5A6172", accent "#8B7CFF",
            bar "#10131A", side "#0E1016", line "#1C2030", strong "#2A3044", code "#141824", code_line "#1F2434",
            syntax ["#FF79C6", "#7CE38B", "#82AAFF", "#FFB86C", "#5C6370", "#C792EA"]),
        theme!("contrast", "High Contrast", true, bg "#000000", fg "#FFFFFF", fg2 "#E0E0E0", fg3 "#B0B0B0", accent "#FFD60A",
            bar "#000000", side "#000000", line "#FFFFFF", strong "#FFFFFF", code "#111111", code_line "#FFFFFF",
            syntax ["#FF8AD8", "#7DFF9B", "#7FC4FF", "#FFD60A", "#C0C0C0", "#D7A8FF"]),
    ]
}

/// Accent swatches offered in Settings, beside "Theme default".
#[allow(dead_code, reason = "the Settings panel offers these")]
pub const ACCENTS: &[(&str, &str)] = &[
    ("Blue", "#2F6BFF"),
    ("Violet", "#7C5CFF"),
    ("Pink", "#E5489B"),
    ("Red", "#E5484D"),
    ("Orange", "#F07A1F"),
    ("Green", "#2E9E5B"),
    ("Teal", "#14A3A3"),
    ("Graphite", "#6B7280"),
];

/// True when every colour in `def` parses as `#rrggbb`.
pub fn valid(def: &ThemeDef) -> bool {
    let s = &def.syntax;
    [&def.bg, &def.fg, &def.fg2, &def.fg3, &def.accent, &def.bar, &def.side, &def.line, &def.line_strong, &def.code_bg, &def.code_line,
        &s.keyword, &s.string, &s.function, &s.number, &s.comment, &s.ty]
        .iter()
        .all(|c| rune::color::hex(c).is_some())
}

/// Reads every `*.json` theme in `dir`, sorted by name. A file that doesn't parse or has a bad
/// colour is skipped, with its name in the returned errors. The id is the file stem, so a user
/// theme named like a built-in one is offered as its own entry.
pub fn load_dir(dir: &Path) -> (Vec<ThemeDef>, Vec<String>) {
    let mut out = Vec::new();
    let mut errors = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else { return (out, errors) };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let parsed = std::fs::read_to_string(&path).ok().and_then(|t| serde_json::from_str::<ThemeDef>(&t).ok());
        match parsed {
            Some(mut def) if valid(&def) => {
                def.id = format!("user:{stem}");
                out.push(def);
            }
            _ => errors.push(format!("{stem}.json")),
        }
    }
    out.sort_by_key(|t| t.name.to_lowercase());
    (out, errors)
}

/// The theme with `id` among the built-ins and `user`, or None.
pub fn find<'a>(id: &str, builtin: &'a [ThemeDef], user: &'a [ThemeDef]) -> Option<&'a ThemeDef> {
    builtin.iter().chain(user).find(|t| t.id == id)
}

#[cfg(test)]
#[path = "themes_tests.rs"]
mod tests;
