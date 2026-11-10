//! Design tokens (PLAN.md §7), the kit they seed, and the Markdown reading style (§4.7).

use rune::{Color, Family, TextStyle};
use rune_kit::syntax::Syntax;
use rune_kit::{Kit, MarkdownStyle, PaletteSeed};

use crate::settings::{Settings, intern};
use crate::themes::{self, ThemeDef};

pub const BAR_H: f32 = 40.0;
pub const ICON: f32 = 16.0;
pub const SIDE_W: f32 = 248.0;
pub const ROW_H: f32 = 28.0;
pub const READ_W: f32 = 720.0;
pub const READ_W_WIDE: f32 = 960.0;

/// Colours the app draws with directly, beside the kit's own tokens.
pub struct Tokens {
    pub bar: Color,
    pub side: Color,
    pub line: Color,
    pub line_strong: Color,
    pub fg: Color,
    pub fg2: Color,
    pub fg3: Color,
    pub hover: Color,
    #[allow(dead_code)] // Hover and press states are not applied yet (NOTES.md).
    pub press: Color,
    pub accent: Color,
    pub accent_soft: Color,
    pub code_bg: Color,
    pub code_line: Color,
}

fn hex(s: &str) -> Color {
    rune::color::hex(s).expect("theme colours are valid #rrggbb literals")
}

/// A theme colour; themes are validated when loaded, so a bad one only reaches here from a
/// built-in typo, which the tests catch. Falls back to white.
fn col_of(s: &str) -> Color {
    rune::color::hex(s).unwrap_or(Color::WHITE)
}

/// Everything a frame draws with: the kit, the app's own tokens and the code colours, for one
/// theme, accent and panel opacity.
pub struct Look {
    #[allow(dead_code, reason = "read by the background and settings views")]
    pub dark: bool,
    pub kit: Kit,
    pub tokens: Tokens,
    /// Keyword, string, function, number, comment, type.
    pub syntax: [Color; 6],
    /// How solid the panels are over a background picture (1.0 without one).
    #[allow(dead_code, reason = "read by the background and settings views")]
    pub panel_alpha: f32,
}

fn build_look(def: &ThemeDef, accent: Option<Color>, panel_alpha: f32) -> Look {
    let fg = col_of(&def.fg);
    let accent = accent.unwrap_or_else(|| col_of(&def.accent));
    let (hover, press) = if def.dark {
        (Color::WHITE.alpha(0.06), Color::WHITE.alpha(0.10))
    } else {
        (fg.alpha(0.055), fg.alpha(0.09))
    };
    let tokens = Tokens {
        bar: col_of(&def.bar).alpha(panel_alpha),
        side: col_of(&def.side).alpha(panel_alpha),
        line: col_of(&def.line),
        line_strong: col_of(&def.line_strong),
        fg,
        fg2: col_of(&def.fg2),
        fg3: col_of(&def.fg3),
        hover,
        press,
        accent,
        accent_soft: accent.alpha(if def.dark { 0.14 } else { 0.10 }),
        code_bg: col_of(&def.code_bg),
        code_line: col_of(&def.code_line),
    };
    let mut kit = Kit::from_palette(PaletteSeed { bg: col_of(&def.bg), fg, accent });
    // The kit decides dark from the seed's contrast; the theme says what it is.
    kit.dark = def.dark;
    kit.c.page = kit.c.page.alpha(panel_alpha);
    let s = &def.syntax;
    let syntax = [&s.keyword, &s.string, &s.function, &s.number, &s.comment, &s.ty].map(|c| col_of(c));
    Look { dark: def.dark, kit, tokens, syntax, panel_alpha }
}

/// The theme `settings` names, resolved against the built-ins and `user` themes. `auto` (and an
/// id that no longer exists) follows the system between `light` and `dark`.
pub fn resolve_theme(settings: &Settings, user: &[ThemeDef], system_dark: bool) -> ThemeDef {
    let builtin = themes::builtin();
    let fallback = if system_dark { "dark" } else { "light" };
    themes::find(&settings.theme, &builtin, user)
        .or_else(|| themes::find(fallback, &builtin, user))
        .cloned()
        .unwrap_or_else(|| builtin[0].clone())
}

/// The look for this frame, built once per distinct theme, accent and panel opacity and kept
/// for the life of the process (a handful of entries: the opacity is rounded to 5%).
pub fn look(settings: &Settings, user: &[ThemeDef], system_dark: bool) -> &'static Look {
    use std::cell::RefCell;
    use std::collections::HashMap;
    type Key = (ThemeDef, Option<String>, u8);
    thread_local! {
        static CACHE: RefCell<HashMap<Key, &'static Look>> = RefCell::new(HashMap::new());
    }
    let def = resolve_theme(settings, user, system_dark);
    let accent = settings.accent.clone().filter(|a| rune::color::hex(a).is_some());
    let pct = (settings.panel_alpha().clamp(0.0, 1.0) * 20.0).round() as u8;
    let key: Key = (def, accent, pct);
    CACHE.with(|c| {
        if let Some(hit) = c.borrow().get(&key) {
            return *hit;
        }
        let look: &'static Look = Box::leak(Box::new(build_look(&key.0, key.1.as_deref().map(col_of), f32::from(pct) / 20.0)));
        c.borrow_mut().insert(key, look);
        look
    })
}

/// The reading style for the document view: the reader's fonts, text size and line spacing,
/// all scaled by `zoom` (1.0 is 100%). Headings keep their §4.7 ratios to the body size.
pub fn markdown_style(look: &Look, settings: &Settings, zoom: f32) -> MarkdownStyle {
    let k = &look.kit;
    let t = &look.tokens;
    let scale = zoom * settings.text_size / crate::settings::TEXT_SIZE;
    // A §4.7 row: size and line-height ratio, weight, tracking as a fraction of the size.
    let font = |size: f32, ratio: f32, weight: f32, tracking_em: f32, base: TextStyle| {
        let s = size * scale;
        TextStyle { size: s, line_height: s * ratio, weight, tracking: s * tracking_em, ..base }
    };
    let mut sans = k.ty.body;
    if !settings.body_font.is_empty() {
        sans.family = Family::Named(intern(&settings.body_font));
    }
    let mono = code_font(k, settings);
    let table_font = font(14.0, 1.5, 400.0, 0.0, sans);
    let [kw, string, func, num, comment, ty] = look.syntax;

    let style = MarkdownStyle::from_kit(k)
        .text(t.fg)
        // Level 4+ headings and list markers. H6 is fg-2 in §4.7; see the TODO below.
        .muted(t.fg)
        .link(t.accent)
        .body_font(font(16.0, settings.line_spacing, 400.0, 0.0, sans))
        .heading_font(1, font(32.0, 1.2, 700.0, -0.02, sans))
        .heading_font(2, font(22.0, 1.3, 650.0, -0.01, sans))
        .heading_font(3, font(18.0, 1.4, 650.0, 0.0, sans))
        // Set after level 3, which also sets level 4; level 4 is 16 / 1.5 here.
        .heading_font(4, font(16.0, 1.5, 650.0, 0.0, sans))
        .code_block_font(font(13.0, 1.65, 400.0, 0.0, mono))
        .inline_code_font(0.875, 400.0)
        .inline_code_bg(t.code_bg)
        .code_block(t.code_bg, t.code_line)
        .quote(t.line_strong, 3.0, 16.0, t.fg2)
        .table(t.line, t.code_bg)
        .table_header_text(t.fg2)
        .table_header_weight(600.0)
        .table_font(table_font)
        .syntax(Syntax::Keyword, kw)
        .syntax(Syntax::String, string)
        .syntax(Syntax::Function, func)
        .syntax(Syntax::Number, num)
        .syntax(Syntax::Comment, comment)
        .syntax(Syntax::Type, ty)
        .highlight_bg(hex("#FFC800").alpha(0.35))
        .highlight_current_bg(hex("#FF9600").alpha(0.70));

    // TODO(upstream): §4.7 lead paragraph (18 / 1.6, fg-2, first paragraph under the H1). MarkdownStyle has no first-block style.
    // TODO(upstream): §4.7 vertical rhythm: 16pt after paragraphs, and heading margins (H2 40 before / 12 after, H3 32 / 8, H4-H6 24 before). No spacing builders.
    // TODO(upstream): H2's 1pt --line underline 8pt below the text. No rule-under-heading builder.
    // TODO(upstream): H6 in fg-2. `muted` covers all of levels 4 to 6, so H4 and H5 are fg.
    // TODO(upstream): table header at 13pt (cells 14pt). `table_font` sets both, so the header uses the cell size.
    // TODO(upstream): table cell padding 8 / 12 and the 8pt outer radius. No public padding builder.
    // TODO(upstream): inline code's 4pt radius and 2 / 5 padding. `inline_code_pad` takes one value.
    // TODO(upstream): code block 8pt radius and the 32pt header row (language and copy). The header belongs to `code_frame` in the app.
    style
}

/// The code font: the reader's family with ligatures on or off, or the kit's monospace (whose
/// ligatures Rune always turns off). A named family is not `Family::Mono`, so Rune leaves its
/// font features alone.
pub fn code_font(k: &Kit, settings: &Settings) -> TextStyle {
    if settings.code_font.is_empty() {
        return k.ty.mono;
    }
    let features = if settings.ligatures { "'liga' 1, 'calt' 1" } else { "'liga' 0, 'calt' 0" };
    TextStyle { family: Family::Named(intern(&settings.code_font)), ..k.ty.mono }.features(features)
}

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
