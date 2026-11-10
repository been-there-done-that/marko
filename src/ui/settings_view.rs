//! The Settings dialog: Appearance, Text, Background and General. Every control writes
//! `AppState::settings` directly, so a change shows in the window behind the dialog at once.

use rune::{Color, El, FileDialogOptions, ScrollSpec, Tx, col, el, row, text};
use rune_kit::{Button, ButtonVariant, Combobox, ComboboxOption, Dialog, Kit, Markdown, SegmentedControl, Switch};

use crate::app::AppState;
use crate::settings::{self, Fit, Settings, Startup, WebImages};
use crate::theme::{self, Look, Tokens};
use crate::themes::{self, ThemeDef};
use crate::ui::slider::Slider;

pub const TABS: [&str; 4] = ["Appearance", "Text", "Background", "General"];
const PAD: f32 = 24.0;
/// The scrolling body's height: the dialog is as tall as its header, tab bar and this.
const BODY_H: f32 = 500.0;
const CARD_W: f32 = 135.0;
const CARD_H: f32 = 80.0;
const CONTROL_W: f32 = 232.0;
/// Combobox id standing for "no choice, use the default" (a font family is never named so).
const DEFAULT_ID: &str = "__default__";

/// Pushes the Settings dialog (and, in the Text tab, an open font list) into `layers`.
pub fn layers(s: &AppState, look: &'static Look, layers: &mut Vec<El>) {
    if !s.settings_open {
        return;
    }
    let k = &look.kit;
    let t = &look.tokens;
    let mut popups = Vec::new();
    let page = match s.settings_tab {
        0 => appearance(s, look),
        1 => text_tab(s, look, &mut popups),
        2 => background(s, look),
        _ => general(s, look),
    };
    let tabs = SegmentedControl::new(&TABS, Some(s.settings_tab.min(TABS.len() - 1))).width(None).label("Settings section")
        .on_pick(|s: &mut AppState, i: usize, _: &mut Tx| s.settings_tab = i)
        .build(k).key("settings-tabs");
    let tab_bar = col().w_full().px(PAD).pb(16.0).child(tabs);
    let body = col().key("settings-body").w_full().h(BODY_H).scroll(ScrollSpec { stick_bottom: false })
        .child(col().key(format!("settings-tab-{}", TABS[s.settings_tab.min(TABS.len() - 1)].to_lowercase())).w_full().px(PAD).py(PAD).gap(PAD).child(page));
    Dialog::new("settings", "Settings")
        .width(640.0)
        .fit()
        .open(true)
        .on_close(|s: &mut AppState| s.settings_open = false)
        .child(tab_bar)
        .child(el().w_full().h(1.0).fill(t.line))
        .child(body)
        .build(k, layers);
    layers.append(&mut popups);
}

// ---- building blocks ----

/// A small section label above a group of rows.
fn section(label: &str, k: &Kit, t: &Tokens) -> El {
    text(label).font(k.ty.small_medium).color(t.fg3)
}

/// A group: its label, then its rows with 4pt between them.
fn group(label: &str, k: &Kit, t: &Tokens, rows: Vec<El>) -> El {
    col().w_full().gap(8.0).child(section(label, k, t)).child(col().w_full().gap(4.0).children(rows))
}

/// One setting: the label (and a hint under it) on the left, its control on the right.
fn setting(label: &str, hint: Option<&str>, control: El, k: &Kit, t: &Tokens) -> El {
    let mut name = col().grow(1.0).shrink().gap(2.0).child(text(label).font(k.ty.ui).color(t.fg));
    if let Some(h) = hint {
        name = name.child(text(h).font(k.ty.caption()).color(t.fg3).wrap());
    }
    row().w_full().min_h(36.0).py(4.0).gap(16.0).center_y().child(name).child(control)
}

fn switch(key: &str, label: &str, on: bool, f: impl Fn(&mut AppState) + 'static, k: &Kit) -> El {
    Switch::new(on).label(label).hide_label().on_toggle(f).build(k).key(key)
}

fn segmented(key: &str, labels: &[&str], at: usize, f: impl Fn(&mut AppState, usize) + Clone + 'static, k: &Kit) -> El {
    SegmentedControl::new(labels, Some(at)).width(Some(CONTROL_W)).label(key)
        .on_pick(move |s: &mut AppState, i: usize, _: &mut Tx| f(s, i))
        .build(k).key(key)
}

/// What a slider row shows: its key and name, the value, `(min, max, step)`, how the value reads, and whether it is off.
struct Sl<'a> {
    key: &'a str,
    label: &'a str,
    v: f32,
    range: (f32, f32, f32),
    unit: fn(f32) -> String,
    off: bool,
}

fn slider(sl: Sl, look: &Look, f: impl Fn(&mut AppState, f32) + 'static) -> El {
    let (min, max, step) = sl.range;
    Slider::new(sl.key, sl.v, min, max, step).label(sl.label).format(sl.unit).disabled(sl.off).on_change(f)
        .build(&look.kit, look.tokens.accent)
}

fn hex_color(h: &str) -> Color {
    rune::color::hex(h).unwrap_or(Color::WHITE)
}

// ---- Appearance ----

fn appearance(s: &AppState, look: &'static Look) -> El {
    let (k, t) = (&look.kit, &look.tokens);
    let cards = std::iter::once(None)
        .chain(themes::builtin().into_iter().chain(s.user_themes.iter().cloned()).map(Some))
        .map(|def| theme_card(s, def, look))
        .collect::<Vec<_>>();
    let grid = row().w_full().flex_wrap().gaps(16.0, 16.0).children(cards);

    let mut swatches = row().gap(10.0).center_y().child(default_swatch(s.settings.accent.is_none(), look));
    for (name, hex) in themes::ACCENTS {
        let on = s.settings.accent.as_deref().is_some_and(|a| a.eq_ignore_ascii_case(hex));
        swatches = swatches.child(swatch(name, hex, on, look));
    }
    let folder = Button::new("Open Themes Folder")
        .on_press(open_themes_folder)
        .build(k).key("open-themes-folder");
    col().w_full().gap(PAD)
        .child(col().w_full().gap(12.0).child(section("Theme", k, t)).child(grid))
        .child(col().w_full().gap(12.0).child(section("Accent", k, t)).child(swatches))
        .child(group("Custom themes", k, t, vec![setting(
            "Your own themes", Some("Drop a .json theme in the themes folder; it shows up above."), folder, k, t)]))
}

/// Creates `<config>/themes` and opens it in the system's file manager.
fn open_themes_folder(s: &mut AppState) {
    let dir = rune::prefs::Prefs::open(crate::APP_ID).path().and_then(|p| p.parent()).map(|d| d.join("themes"));
    match dir {
        Some(dir) if std::fs::create_dir_all(&dir).is_ok() => super::document::open_with_os(&dir),
        _ => {
            let now = s.now;
            s.show_toast("Couldn't open the themes folder", now);
        }
    }
}

/// A theme preview: its page, a sidebar strip, two text lines and an accent dot. `None` is the
/// Follow System card, half light and half dark.
fn theme_card(s: &AppState, def: Option<ThemeDef>, look: &'static Look) -> El {
    let (k, t) = (&look.kit, &look.tokens);
    let (id, name) = match &def {
        Some(d) => (d.id.clone(), d.name.clone()),
        None => (settings::AUTO_THEME.to_owned(), "Follow System".to_owned()),
    };
    let on = s.settings.theme == id;
    let art = match def {
        Some(d) => mini(&d, CARD_W - 4.0, CARD_H - 4.0),
        None => {
            let builtin = themes::builtin();
            let (w, h) = (CARD_W - 4.0, CARD_H - 4.0);
            let half = w / 2.0;
            let (light, dark) = (&builtin[0], &builtin[1]);
            let right = el().absolute(half, 0.0).w(half).h(h).fill(hex_color(&dark.bg)).child(
                col().px(8.0).py(12.0).gap(6.0)
                    .child(el().w(28.0).h(5.0).radius(2.5).fill(hex_color(&dark.fg)))
                    .child(el().w(36.0).h(5.0).radius(2.5).fill(hex_color(&dark.fg2)))
                    .child(el().size(10.0, 10.0).radius(5.0).fill(hex_color(&dark.accent)).mt(4.0)),
            );
            el().w(w).h(h).child(mini(light, w, h)).child(right)
        }
    };
    let ring = if on { t.accent } else { t.line };
    let frame = el().w(CARD_W).h(CARD_H).radius(10.0).padding(if on { 0.0 } else { 1.0 }).border(if on { 2.0 } else { 1.0 }, ring)
        .clip_rounded(10.0).child(art);
    let key = format!("theme-card-{}", id.replace(':', "-"));
    let pick = id;
    col().key(key).w(CARD_W).gap(6.0).label(name.clone())
        .child(frame)
        .child(text(name).font(if on { k.ty.small_medium } else { k.ty.small }).color(if on { t.fg } else { t.fg2 }).truncate())
        .on_press(move |s: &mut AppState| s.settings.theme = pick.clone())
}

/// Two text lines and an accent dot in `d`'s colours.
fn page_lines(d: &ThemeDef, h: f32) -> El {
    let line = |frac: f32, c: Color| el().w_frac(frac).h(5.0).radius(2.5).fill(c);
    col().h(h).px(10.0).py(12.0).gap(6.0)
        .child(line(0.55, hex_color(&d.fg)))
        .child(line(0.85, hex_color(&d.fg2)))
        .child(line(0.7, hex_color(&d.fg2)))
        .child(el().size(10.0, 10.0).radius(5.0).fill(hex_color(&d.accent)).mt(4.0))
}

/// The little window drawn inside a theme card.
fn mini(d: &ThemeDef, w: f32, h: f32) -> El {
    let page = page_lines(d, h).grow(1.0);
    row().w(w).h(h).fill(hex_color(&d.bg))
        .child(el().w(30.0).h(h).fill(hex_color(&d.side)))
        .child(el().w(1.0).h(h).fill(hex_color(&d.line)))
        .child(page)
}

fn swatch(name: &str, hex: &str, on: bool, look: &Look) -> El {
    let t = &look.tokens;
    let dot = el().size(20.0, 20.0).radius(10.0).fill(hex_color(hex));
    let ring = el().size(28.0, 28.0).radius(14.0).center().border(2.0, if on { t.fg } else { Color::TRANSPARENT }).child(dot);
    let hex = hex.to_owned();
    ring.key(format!("accent-{}", name.to_lowercase())).label(name.to_owned()).focusable(true)
        .on_press(move |s: &mut AppState| s.settings.accent = Some(hex.clone()))
}

/// "Theme default": a hollow ring, selected when no accent is chosen.
fn default_swatch(on: bool, look: &Look) -> El {
    let t = &look.tokens;
    let dot = el().size(20.0, 20.0).radius(10.0).border(1.5, t.fg3).center().child(el().size(2.0, 2.0).radius(1.0).fill(t.fg3));
    el().size(28.0, 28.0).radius(14.0).center().border(2.0, if on { t.fg } else { Color::TRANSPARENT }).child(dot)
        .key("accent-default").label("Theme default").focusable(true)
        .on_press(|s: &mut AppState| s.settings.accent = None)
}

// ---- Text ----

fn font_options(list: &[String]) -> Vec<ComboboxOption> {
    std::iter::once(ComboboxOption::new(DEFAULT_ID, "Default"))
        .chain(list.iter().map(|f| ComboboxOption::new(f.clone(), f.clone())))
        .collect()
}

/// One font picker: its key, the family now chosen, the families offered and where its state lives.
struct Pick<'a> {
    key: &'a str,
    current: &'a str,
    list: &'a [String],
    loaded: bool,
    state: &'a rune_kit::ComboboxState,
    access: fn(&mut AppState) -> &mut rune_kit::ComboboxState,
    choose: fn(&mut Settings, &str),
}

fn font_picker(p: Pick, k: &Kit, popups: &mut Vec<El>) -> El {
    let (access, choose) = (p.access, p.choose);
    let opts = font_options(p.list);
    let cur = if p.current.is_empty() { DEFAULT_ID } else { p.current };
    Combobox::new(p.key, p.state, &opts, access)
        .label(p.key.replace('-', " "))
        .current(Some(cur))
        .placeholder(if p.loaded { "Default" } else { "Loading fonts…" })
        .search_placeholder("Search fonts")
        .width(CONTROL_W)
        .disabled(!p.loaded)
        .on_choose(move |s: &mut AppState, id: &str, _: &mut Tx| {
            choose(&mut s.settings, if id == DEFAULT_ID { "" } else { id });
            access(s).close();
        })
        .build(k, popups)
}

fn text_tab(s: &AppState, look: &'static Look, popups: &mut Vec<El>) -> El {
    let (k, t) = (&look.kit, &look.tokens);
    let st = &s.settings;
    let body = font_picker(Pick { key: "body-font", current: &st.body_font, list: &s.fonts.all, loaded: s.fonts.loaded,
        state: &s.body_combo, access: |s| &mut s.body_combo, choose: |st, v| st.body_font = v.to_owned() }, k, popups);
    let code = font_picker(Pick { key: "code-font", current: &st.code_font, list: &s.fonts.mono, loaded: s.fonts.loaded,
        state: &s.code_combo, access: |s| &mut s.code_combo, choose: |st, v| st.code_font = v.to_owned() }, k, popups);
    let fonts = group("Fonts", k, t, vec![
        setting("Body font", None, body, k, t),
        setting("Code font", None, code, k, t),
        setting("Ligatures", Some("Joins => and != where the code font has them"),
            switch("ligatures", "Ligatures", st.ligatures, |s| s.settings.ligatures = !s.settings.ligatures, k), k, t),
    ]);
    let size = slider(Sl { key: "text-size", label: "Text size", v: st.text_size,
        range: (settings::TEXT_SIZE_MIN, settings::TEXT_SIZE_MAX, 1.0), unit: |v| format!("{v:.0} pt"), off: false }, look,
        |s, v| s.settings.text_size = v);
    let spacing = slider(Sl { key: "line-spacing", label: "Line spacing", v: st.line_spacing,
        range: (settings::LINE_SPACING_MIN, settings::LINE_SPACING_MAX, 0.1), unit: |v| format!("{v:.1}"), off: false }, look,
        |s, v| s.settings.line_spacing = v);
    let reading = group("Reading", k, t, vec![
        setting("Text size", None, size, k, t),
        setting("Line spacing", None, spacing, k, t),
        setting("Wide reading column", Some("A broader page for tables and code"),
            switch("wide", "Wide reading column", s.wide, |s| s.wide = !s.wide, k), k, t),
    ]);
    col().w_full().gap(PAD).child(fonts).child(reading).child(sample(s, look))
}

/// A short document drawn with the current fonts and sizes.
fn sample(s: &AppState, look: &'static Look) -> El {
    let (k, t) = (&look.kit, &look.tokens);
    let src = "### The quick brown fox\n\nJumps over the lazy dog, set in your chosen fonts.\n\n```rust\nlet ok = a != b && c => d;\n```";
    let md = Markdown::new(src).style(theme::markdown_style(look, &s.settings, 1.0)).build(k);
    col().w_full().gap(8.0).child(section("Preview", k, t))
        .child(col().key("settings-sample").w_full().padding(20.0).radius(8.0).fill(k.c.page).border(1.0, t.line).child(md))
}

// ---- Background ----

fn background(s: &AppState, look: &'static Look) -> El {
    let (k, t) = (&look.kit, &look.tokens);
    let b = &s.settings.background;
    let has = b.path.is_some();
    let name = b.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
    let choose = Button::new("Choose Picture…").on_press_tx(choose_picture).build(k).key("bg-choose");
    let remove = Button::new("Remove").variant(ButtonVariant::Ghost).disabled(!has)
        .on_press(|s: &mut AppState| s.settings.background.path = None).build(k).key("bg-remove");
    let buttons = row().gap(8.0).child(choose).child(remove);
    let pct = |v: f32| format!("{v:.0}%");
    let picture = group("Picture", k, t, vec![
        setting("Background picture", Some(name.as_deref().unwrap_or("None chosen")), buttons, k, t),
        setting("Fit", None, segmented("bg-fit", &["Cover", "Contain"], usize::from(b.fit == Fit::Contain),
            |s, i| s.settings.background.fit = if i == 1 { Fit::Contain } else { Fit::Cover }, k), k, t),
    ]);
    let image = slider(Sl { key: "bg-image-opacity", label: "Picture opacity", v: b.image_opacity * 100.0, range: (0.0, 100.0, 5.0), unit: pct, off: !has }, look,
        |s, v| s.settings.background.image_opacity = v / 100.0);
    let panel = slider(Sl { key: "bg-panel-opacity", label: "Panel opacity", v: b.panel_opacity * 100.0,
        range: (settings::PANEL_MIN * 100.0, 100.0, 5.0), unit: pct, off: !has }, look,
        |s, v| s.settings.background.panel_opacity = v / 100.0);
    let blur = slider(Sl { key: "bg-blur", label: "Blur", v: b.blur, range: (0.0, settings::BLUR_MAX, 1.0), unit: |v| format!("{v:.0} pt"), off: !has }, look,
        |s, v| s.settings.background.blur = v);
    let mut look_rows = vec![
        setting("Picture opacity", None, image, k, t),
        setting("Panel opacity", Some("How solid the sidebar and page are over the picture"), panel, k, t),
        setting("Blur", Some("Frosted glass behind the panels"), blur, k, t),
    ];
    if !has {
        look_rows.insert(0, text("Choose a picture to adjust these.").font(k.ty.caption()).color(t.fg3).mb(4.0));
    }
    col().w_full().gap(PAD).child(picture).child(group("Look", k, t, look_rows))
}

fn choose_picture(_: &mut AppState, tx: &mut Tx) {
    let options = FileDialogOptions { title: "Choose a Background Picture".into(), ..Default::default() }
        .filter("Pictures", &["png", "jpg", "jpeg", "webp", "gif"]);
    tx.open_file(options, |s: &mut AppState, r, _: &mut Tx| {
        if let Ok(Some(path)) = r {
            s.settings.background.path = Some(path);
        }
    });
}

// ---- General ----

fn general(s: &AppState, look: &'static Look) -> El {
    let (k, t) = (&look.kit, &look.tokens);
    let st = &s.settings;
    let images = segmented("web-images", &["Ask", "Always"], usize::from(st.web_images == WebImages::Always),
        |s, i| s.settings.web_images = if i == 1 { WebImages::Always } else { WebImages::Ask }, k);
    let startup = segmented("startup", &["Restore", "Start empty"], usize::from(st.startup == Startup::Empty),
        |s, i| s.settings.startup = if i == 1 { Startup::Empty } else { Startup::Restore }, k);
    let reset = Button::new("Reset to Defaults")
        .on_press(|s: &mut AppState| s.settings = Settings::default()).build(k).key("settings-reset");
    col().w_full().gap(PAD)
        .child(group("Documents", k, t, vec![
            setting("Web images", Some("Loading one tells its server you opened the file"), images, k, t),
            setting("Show reading progress", None,
                switch("progress", "Show reading progress", st.show_progress, |s| s.settings.show_progress = !s.settings.show_progress, k), k, t),
        ]))
        .child(group("App", k, t, vec![
            setting("On launch", Some("Restore reopens last session's tabs"), startup, k, t),
            setting("Show memory and CPU", Some("In the titlebar"),
                switch("stats", "Show memory and CPU", s.stats_on, |s| s.stats_on = !s.stats_on, k), k, t),
        ]))
        .child(el().w_full().h(1.0).fill(t.line))
        .child(row().w_full().justify_end().child(reset))
}

#[cfg(test)]
#[path = "settings_view_tests.rs"]
mod tests;
