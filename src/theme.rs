//! Design tokens (PLAN.md §7), the kit they seed, and the Markdown reading style (§4.7).

use std::sync::OnceLock;

use rune::{Color, TextStyle};
use rune_kit::syntax::Syntax;
use rune_kit::{Kit, MarkdownStyle, PaletteSeed};

pub const BAR_H: f32 = 40.0;
pub const ICON: f32 = 16.0;
pub const SIDE_W: f32 = 248.0;
pub const ROW_H: f32 = 28.0;
pub const READ_W: f32 = 720.0;
pub const READ_W_WIDE: f32 = 960.0;

/// Colours the app draws with directly, beside the kit's own tokens.
pub struct Tokens {
    #[allow(dead_code)] // The titlebar fill is not themed yet (NOTES.md).
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

fn build_tokens(dark: bool) -> Tokens {
    if dark {
        let fg = hex("#EDEDEF");
        let accent = hex("#6B93FF");
        Tokens {
            bar: hex("#1F1F23"),
            side: hex("#1C1C20"),
            line: hex("#2C2C31"),
            line_strong: hex("#3A3A40"),
            fg,
            fg2: hex("#A1A1AA"),
            fg3: hex("#63636B"),
            hover: Color::WHITE.alpha(0.06),
            press: Color::WHITE.alpha(0.10),
            accent,
            accent_soft: accent.alpha(0.14),
            code_bg: hex("#1F1F23"),
            code_line: hex("#2C2C31"),
        }
    } else {
        let fg = hex("#1C1917");
        let accent = hex("#2F6BFF");
        Tokens {
            bar: hex("#F5F5F4"),
            side: hex("#FAFAF9"),
            line: hex("#E7E5E4"),
            line_strong: hex("#D6D3D1"),
            fg,
            fg2: hex("#57534E"),
            fg3: hex("#A8A29E"),
            hover: fg.alpha(0.055),
            press: fg.alpha(0.09),
            accent,
            accent_soft: accent.alpha(0.10),
            code_bg: hex("#F7F7F6"),
            code_line: hex("#ECEBEA"),
        }
    }
}

/// The token set for the light or dark appearance, built once.
pub fn tokens(dark: bool) -> &'static Tokens {
    static LIGHT: OnceLock<Tokens> = OnceLock::new();
    static DARK: OnceLock<Tokens> = OnceLock::new();
    if dark { DARK.get_or_init(|| build_tokens(true)) } else { LIGHT.get_or_init(|| build_tokens(false)) }
}

/// The kit for the light or dark appearance, built once per appearance.
pub fn kit(dark: bool) -> &'static Kit {
    static LIGHT: OnceLock<Kit> = OnceLock::new();
    static DARK: OnceLock<Kit> = OnceLock::new();
    if dark {
        DARK.get_or_init(|| Kit::from_palette(PaletteSeed { bg: hex("#18181B"), fg: hex("#EDEDEF"), accent: hex("#6B93FF") }))
    } else {
        LIGHT.get_or_init(|| Kit::from_palette(PaletteSeed { bg: hex("#FFFFFF"), fg: hex("#1C1917"), accent: hex("#2F6BFF") }))
    }
}

/// The reading style for the document view at `zoom` (1.0 is 100%). Zoom scales every size
/// and line height in §4.7.
pub fn markdown_style(dark: bool, zoom: f32) -> MarkdownStyle {
    let k = kit(dark);
    let t = tokens(dark);
    // A §4.7 row: size and line-height ratio, weight, tracking as a fraction of the size.
    let font = |size: f32, ratio: f32, weight: f32, tracking_em: f32, base: TextStyle| {
        let s = size * zoom;
        TextStyle { size: s, line_height: s * ratio, weight, tracking: s * tracking_em, ..base }
    };
    let sans = k.ty.body;
    let mono = k.ty.mono;
    let table_font = font(14.0, 1.5, 400.0, 0.0, sans);

    let style = MarkdownStyle::from_kit(k)
        .text(t.fg)
        // Level 4+ headings and list markers. H6 is fg-2 in §4.7; see the TODO below.
        .muted(t.fg)
        .link(t.accent)
        .body_font(font(16.0, 1.7, 400.0, 0.0, sans))
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
        .syntax(Syntax::Keyword, if dark { hex("#FF7AB8") } else { hex("#B4237A") })
        .syntax(Syntax::String, if dark { hex("#7EE2A0") } else { hex("#1A7F37") })
        .syntax(Syntax::Function, if dark { hex("#8FB2FF") } else { hex("#2F5FD0") })
        .syntax(Syntax::Number, if dark { hex("#F5B86B") } else { hex("#A15C00") })
        .syntax(Syntax::Comment, if dark { hex("#73737D") } else { hex("#8C8781") })
        .syntax(Syntax::Type, if dark { hex("#C7A1FF") } else { hex("#7A3FC2") })
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

#[cfg(test)]
#[path = "theme_tests.rs"]
mod tests;
