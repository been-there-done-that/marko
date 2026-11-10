# Phase 3: Make it yours — implementation plan

> For agentic workers: each task below runs in its own git worktree, in parallel. Read the
> shared brief first. Steps use checkboxes.

**Goal:** fonts, themes, a whole-app background picture, a Settings panel, better Markdown
images, a command palette and window memory.

**Already done on `main` (the foundation; build on it, don't redo it):**
- `src/settings.rs`: `Settings` (theme, accent, body_font, code_font, ligatures, text_size,
  line_spacing, background {path, image_opacity, panel_opacity, blur, fit}, web_images,
  show_progress, startup), `clamped()`, `panel_alpha()`, `intern()`, `initial()`. Saved in
  `Session::settings` and read before the first frame. `AppState::settings`.
- `src/themes.rs`: `ThemeDef` (hex strings), `builtin()` (light, dark, paper, solar-light,
  solar-dark, nordic, midnight, contrast), `ACCENTS`, `load_dir` (user `*.json` →
  id `user:<stem>`), `find`. `AppState::user_themes`, loaded from `<config>/themes` in `start`.
- `src/theme.rs`: `Look { dark, kit, tokens, syntax, panel_alpha }`;
  `theme::look(&s.settings, &s.user_themes, system_dark) -> &'static Look` (cached);
  `markdown_style(look, settings, zoom)`; `code_font(kit, settings) -> TextStyle` (named
  family + ligature features). Every view takes `look: &'static Look` (k = `&look.kit`,
  t = `&look.tokens`). With a background picture, `tokens.bar`, `tokens.side` and
  `kit.c.page` carry `panel_alpha`.
- `src/fonts.rs`: `FontList { all, mono, loaded }`, `AppState::fonts` (empty until loaded).
- More menu: a "Theme" group lists Follow System + every theme.

## Global constraints
- Shared brief: the scratchpad `agent-brief.md` (path given in your prompt). Rune API facts: `rune-api-notes.md` next to it.
- Rune source is read-only at `/tmp/rune/crates/{rune,rune-kit}/src`. Pinned rev a7f276c.
- Don't change `settings.rs`, `themes.rs` or `theme.rs` public signatures; you may add to them.
- Add AppState fields at the end of the struct and of `AppState::new`, under a comment naming your task.
- New `poll` work goes in one call to a function in your own module (`yourmod::tick(s, now, tx) -> bool`), added on its own line in `poll`.
- New overlay layers go in one line in `ui::view` calling your module.
- Every visible change gets a `--snap` scene in `SCENES` (main.rs) and you look at the PNG.
- No new dependency except the ones a task names.

## Task A — Installed fonts and the code font in the editor
Files: `src/fonts.rs` (+ `fonts_tests.rs`), `src/main.rs` (spawn loader in `start`), `src/ui/editor.rs` (+ tests).
- [ ] `pub fn parse_fc_list(out: &str) -> Vec<String>`: one family per line (take text before the first `,`), trimmed, skip empty and `.`-prefixed, dedupe case-insensitively, sort case-insensitively. Test with a sample containing dupes, a hidden family and a comma list.
- [ ] `pub fn load() -> FontList`: Linux runs `fc-list --format='%{family}\n'` and `fc-list :spacing=mono --format='%{family}\n'` (100 = mono, also try `:spacing=100`), `loaded: true`. Other OSes: empty lists, `loaded: true` (note it in NOTES.md).
- [ ] `start()` spawns a thread `fonts` that calls `load()` and lands it with `handle.update(|s,_| s.fonts = list)`.
- [ ] Editor: make the editing field draw in `theme::code_font(k, &s.settings)` scaled by `s.zoom` (size 13×zoom×text_size/16, line height 1.6×). rune-kit `TextArea` hard-codes its font, so use rune's `text_input(&TextEdit, style)` (rune/src/el.rs:730) with the same edit wiring the TextArea gives (read rune-kit/src/text_input.rs ~480-560 to copy what TextArea sets up: on_edit, multiline, scroll, focus). Keep every existing editor test green. Add `pub fn editor_style(look, settings, zoom) -> TextStyle` with a test that a chosen code font reaches it.
- [ ] Scene `editor-font` (code_font = "DejaVu Sans Mono", ligatures on). Commit "fonts: installed font list; editor draws in the code font".

## Task B — Background picture
Files: `src/background.rs` (+ tests), `src/app.rs` (field), `src/worker.rs` (job), `src/main.rs` (tick line, scene), `src/ui/mod.rs` (draw), and the panel views only where a fill must change.
- [ ] State `pub struct BackgroundState { pub image: Option<rune::Image>, pub for_path: Option<PathBuf>, pub failed: bool }`, field `AppState::bg`.
- [ ] `background::tick(s, now, tx) -> bool`: when `settings.background.path != bg.for_path`, set `for_path`, clear the image, queue `Job::Background { path }` (none when path is None). The worker decodes with `images::decode(path, 3840)` and returns `JobResult::Background { path, image }`; `s.background_ready` stores it only if `path` still matches; a failure sets `failed` and shows the toast "Couldn't load the background picture".
- [ ] Draw: in `ui::view`, when the image is ready, wrap the base so the picture sits behind everything: an element pinned to all four edges with `.pixels_fit(img.pixels(), SurfaceFit::Fill /*Cover*/ or ::Fit /*Contain*/)` and `.opacity(image_opacity)`, then the base over it. `base` must not paint an opaque fill over it: `k.c.page` already carries panel alpha, so check every full-size fill between the root and the panels (shell, body, doc area, sidebar, titlebar, editor) and make sure the picture is covered by exactly one translucent layer (no doubled alpha). Where `settings.background.blur > 0`, add `.backdrop_blur(blur)` on the sidebar, titlebar and document area (Rune allows 4 per frame).
- [ ] Theme dialogs/menus/popovers stay opaque (they use other palette slots; verify in a snap).
- [ ] Tests: tick queues one job per path change and none when unchanged; a stale result is dropped; the root contains key `bg-image` only when an image is ready.
- [ ] Scene `background` using a fixture picture (add a small `fixtures/bg.jpg`, ≤ 200 KB, generate it with the `image` crate in a test helper or commit a simple gradient). Look at it in light and dark. Commit "background: picture behind the whole window with opacity, frosted panels".

## Task C — Settings panel
Files: `src/ui/settings_view.rs` (+ tests), `src/ui/slider.rs` (+ tests), `src/app.rs` (field `settings_open: bool`, `settings_tab: usize`), `src/ui/mod.rs` (layer line), `src/ui/keys.rs` (Ctrl+,), `src/ui/menus.rs` ("Settings…" item at top of More menu, key `menu-settings`), `src/ui/document.rs` (honour `show_progress`).
- [ ] Slider (no rune-kit slider exists): `Slider::new(key, value, min, max, step).label(..).format(fn(f32)->String).on_change(Fn(&mut S, f32))` built from `El::on_drag` (rune/src/el/handlers.rs:32, `Drag{x,w,phase}`) plus `on_step` for arrow keys. Track, filled part in accent, round thumb, value text on the right. Pure `fn value_at(x, w, min, max, step) -> f32` with tests (snaps to step, clamps).
- [ ] Dialog "Settings" (rune-kit `Dialog`, width 640, opened by `s.settings_open`), a `SegmentedControl` of Appearance | Text | Background | General (`settings_tab`). Every control writes `s.settings` directly; changes are live.
  - Appearance: theme cards in a wrapping grid (Follow System first, then built-ins, then user themes): a 120×72 preview filled with the theme's bg, a bar of its side colour, two text lines in fg/fg2 and an accent dot; name below; accent ring when selected. Key `theme-card-<id>` (`:` → `-`). Accent row: "Theme default" + `themes::ACCENTS` swatches (key `accent-<name lowercase>`). Button "Open Themes Folder" (creates `<config>/themes` and opens it with the OS opener; reuse the opener in document.rs by making it `pub(crate)`).
  - Text: Body font and Code font pickers (rune-kit `Combobox` over `s.fonts.all` / `s.fonts.mono`, first option "Default"; "Loading fonts…" while `!s.fonts.loaded`), Ligatures switch, Text size slider 12–24 step 1 ("16 pt"), Line spacing 1.2–2.2 step 0.1, Wide reading column switch (`s.wide`), and a small live sample (a heading, a sentence and a code line drawn with the current style).
  - Background: "Choose Picture…" (`tx.open_file` with png/jpg/jpeg/webp/gif filter, sets `background.path`), "Remove", Picture opacity 0–100%, Panel opacity 35–100%, Blur 0–40, Fit segmented Cover | Contain. Sliders disabled-looking (40% opacity, no handlers) without a picture.
  - General: Web images Ask | Always, Show reading progress switch, On launch Restore last session | Start empty, Show memory and CPU switch (`s.stats_on`), "Reset to Defaults" button (resets `s.settings` to default but keeps `background.path` = None).
- [ ] Esc and the close button close it; Ctrl+, toggles.
- [ ] Tests: Ctrl+, opens; tab switching; pressing a theme card sets `settings.theme`; accent swatch sets `settings.accent`; reset restores defaults; slider drag maths.
- [ ] Scenes `settings-appearance`, `settings-text`, `settings-background`, `settings-general`. Look at each. Commit "settings: panel for theme, accent, fonts, text, background and general".

## Task D — Markdown images
Files: `src/images.rs` (+ tests), `src/net.rs` (+ tests, new), `src/worker.rs`, `src/app.rs`, `src/ui/document.rs`, `src/ui/lightbox.rs` (new), `src/ui/mod.rs` (layer line), `src/ui/keys.rs` (Esc closes lightbox), `Cargo.toml` (add `ureq = { version = "2", default-features = false, features = ["tls"] }`).
- [ ] SVG: `decode` handles `.svg`/`.svgz` through rune's ImageCache SVG path (`ImageCache::svg(bytes, size_pt, scale)` rune/src/image.rs:189, or `file()` which picks by extension), sized to the column width. Test with a tiny SVG fixture.
- [ ] Web images: `net::fetch(url) -> Result<Vec<u8>, String>` (15 s timeout, 20 MB cap, http/https only, a generic User-Agent `marko`). Worker job `Job::WebImage { url }` → decode from bytes (factor a `decode_bytes`), stored in the cache under a key for the URL. Policy: `settings.web_images == Always` queues fetches when a doc shows; `Ask` shows a banner at the top of the document "This document has N web images" with a "Load" button (key `doc-web-load`) that allows them for that tab (`Tab`-level or a `HashSet<u64>` of allowed tab ids). Until loaded they show alt text (the image hook can't draw a placeholder). Never fetch without Always or a click. Tests: banner shows count; no job queued under Ask until Load; Always queues.
- [ ] Refresh on change: when a watched file event names a cached local image, evict it and requeue (check `watch.rs`/`file_event` to see which paths arrive; images in the doc's folder already are). Test with `file_event`.
- [ ] Editor preview: images referenced in the edit buffer load as the text changes (queue missing ones when the buffer changes; reuse `image_srcs`). Test.
- [ ] Lightbox: `Markdown::image_frame` (rune-kit markdown/image.rs:53) wraps each image with `.on_press` that sets `s.lightbox = Some(key)`; a layer with a dark scrim and the picture fit to 90% of the window; click or Esc closes. Key `lightbox`. Test press opens, Esc closes.
- [ ] Scenes `images-svg`, `lightbox`. Commit "images: SVG, web images on request, refresh on change, editor preview, full-size view".

## Task E — Command palette and window memory
Files: `src/ui/palette.rs` (+ tests), `src/commands.rs` (+ tests), `src/app.rs` (fields), `src/ui/mod.rs` (layer line), `src/ui/keys.rs` (Ctrl+Shift+P, Ctrl+K as alias if free), `src/main.rs` (window memory), `src/session.rs` (frame capture).
- [ ] `commands.rs`: `pub struct Command { pub id: String, pub label: String, pub hint: Option<String> }`, `pub fn all(s: &AppState) -> Vec<Command>` (Open File, Open Folder, New File, Save, Toggle Sidebar, Find, Preview/Edit, Zoom In/Out/Actual, Wide column, Settings (sets `s.settings_open = true` — add that bool if Task C hasn't; same name), every theme "Theme: <name>", every recent file "Open Recent: <name>"), `pub fn run(s, id, tx)`, and `pub fn fuzzy(query, label) -> Option<i32>` (subsequence match, case-insensitive, scores word-start and consecutive hits higher). Tests for fuzzy ranking and that each id runs.
- [ ] Palette: a top-centred panel (560 wide) with a focused `TextInput`, the best 12 matches, ↑/↓ to move, Enter runs, Esc closes (handle Enter/Esc/arrows with `TextInput::on_key`, see `keys::find_field_key`). Keys `palette`, `palette-input`, `palette-row-<i>`.
- [ ] Window memory: save the window's position and size in `Session::frame` and reopen there. In `main`, keep a clone of the `WindowHandle` from `on_window`/`on_ready` (UI-thread only; store in a `thread_local!`), read `placement()` in `poll` only when the debounce is about to save, and at launch pass `App::window_options(WindowOptions { position, physical_size, .. })` from the saved frame when it fits a sane range (size ≥ min, position not absurd). Unit-test the range check.
- [ ] Scene `palette` (query "the"). Commit "palette: command palette; window remembers its size and place".
