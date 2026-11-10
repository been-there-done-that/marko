# Marko — Phase 1a (App Shell) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. If you are not Claude Code (opencode, Codex, …), just work the tasks in order: one task, its tests green, one commit, then the next.

**Goal:** A working Markdown viewer window. It has the 40pt custom titlebar (tabs, sidebar toggle, Open menu, mode switch, find and more buttons), opens files and folders through dialogs, drag-and-drop and the CLI, shows a folder sidebar, renders the active tab with rune-kit `Markdown`, and restores the last session.

**Architecture:**
- A single Rust binary crate on Rune + rune-kit.
- All logic lives in plain modules with unit tests: paths, loading, tabs, sidebar roots, the open model and the session. The views stay thin.
- View behaviour is tested headlessly with `rune::Runtime` (press keyed elements, send keys, drop files).
- Slow work (reading files, scanning folders) runs on worker threads and comes back through `AppHandle::update`.

**Tech Stack:**
- Rust 2024 edition.
- `rune` and `rune-kit` (git, pinned rev).
- `serde` + `serde_json`.
- `ignore` (folder walking).
- `tempfile` (dev only).

**Spec:** `PLAN.md` in this repo root. Read §1–§7 before starting; section numbers below refer to it. The UI reference is `ui-preview.html`: open it in a browser.

**Not in this plan (later plans):**
- Find (§4.8), images (§5.3), live reload (§5.5), code-block copy button, link anchors (phase 1b/1c).
- Toast polish beyond the two toasts named here.
- The Linux client-side titlebar (phase 0, a Rune PR).

## Global Constraints

- Rune dependency (exactly, both crates; same rev):
  - `rune = { git = "https://github.com/anomalyco/rune", rev = "a7f276c3630f85a06a0230295da4d2bd172d8d3b", features = ["native-file-dialog"] }`
  - `rune-kit = { git = "https://github.com/anomalyco/rune", rev = "a7f276c3630f85a06a0230295da4d2bd172d8d3b", features = ["markdown", "mermaid"] }`
- Never `[patch]` Rune on the main branch.
- When you need to know how a Rune API behaves, read its source in a local clone of that repo at that rev:
  - `git clone https://github.com/anomalyco/rune /tmp/rune-src && git -C /tmp/rune-src checkout a7f276c`
  - Paths below such as `rune/src/app.rs:173` mean `crates/rune/src/app.rs` there.
  - Mirror patterns from `examples/showcase/src/main.rs`, which uses a custom titlebar, Kit caching, `rune_kit::root` and `on_ready`.
- `AppSpec::init` and `AppSpec::view` are plain `fn` pointers, **not closures**. Pass startup data (CLI paths) through `App::on_ready` + `AppHandle::update`, never through captured variables.
- Titlebar height **40pt**, window default **1100×760**, min **560×400**.
- Markdown extensions accepted (case-insensitive): `md`, `markdown`, `mdown`, `mkd`, `txt`.
- Folder scan ignores: `.git`, `node_modules`, `target`, `.venv`, every dot-folder, and `.gitignore` rules.
- Size limits: refuse files > **50 MB**; mark files > **5 MB** as large; refuse binary files (a NUL byte in the first **8192** bytes).
- Recent list: at most **16** entries, newest first, de-duplicated by canonical path.
- Prefs app id: **`dev.marko.app`** (was `dev.rune.markdown`; migrated on first launch).
- Linux (decision D10): at the very top of `main`, before anything else, remove `WAYLAND_DISPLAY` from the environment so winit uses X11 (drag-and-drop works there).
- Linux (D11 fallback): keep the native frame. Use **no** `borderless`. The 40pt band sits under the system title bar until the phase-0 Rune PR lands.
- No UI thread blocking: no `std::fs` read inside `view` or a press handler. Handlers call into the worker (Task 5) or into pure functions on data already in memory.
- Rune house rules apply to this repo:
  - source files under 800 lines;
  - one feature per module;
  - each `mod` / `pub use` list one item per line, sorted.
- Commit after every task with a message `area: what changed` (e.g. `tabs: dedupe by canonical path`).

## Review Focus

1. **The same file reached by two spellings** (`./a.md`, `a.md`, a symlink to `a.md`) must give one tab, not two. Covered by `tabs::tests::open_dedupes_by_canonical_path` (Task 3).
2. **Paths in the saved session that no longer exist** must be skipped silently on restore, and dropped from Recent. They must not open as error tabs or panic. Covered by `session::tests::restore_skips_missing_paths` (Task 6).
3. **A drop mixing a folder, a Markdown file and a `.png`** must give the folder → sidebar, the file → tab, and the png → the "Skipped 1 file that isn't Markdown" toast. Covered by `open::tests::mixed_drop_routes_each_kind` (Task 5) and `view_tests::drop_mixed_paths` (Task 10).
4. **Closing the active tab** must activate its right neighbour, or its left neighbour when it was last, and closing the last tab must show the empty state. Covered by `tabs::tests::close_active_picks_right_then_left` (Task 3) and `view_tests::closing_last_tab_shows_empty_state` (Task 10).
5. **A huge folder** (`node_modules` with 50k files, or a home directory) must not freeze the window. The scan runs off-thread, ignores the excluded folders, and stops at **20,000** Markdown files. Covered by `sidebar::tests::scan_skips_ignored_and_caps` (Task 4).

---

## File map

```
Cargo.toml
src/main.rs            # Task 9: WAYLAND_DISPLAY, prewarm, App wiring, on_ready
src/app.rs             # Task 5: AppState + update entry points (pure)
src/open.rs            # Task 5: classify + route opened paths
src/paths.rs           # Task 1: is_markdown, canonical, display helpers
src/load.rs            # Task 2: read a file into DocState
src/tabs.rs            # Task 3: TabSet
src/sidebar.rs         # Task 4: roots, scan, tree build
src/session.rs         # Task 6: Session (serde), restore/save, instance lock
src/icons.rs           # Task 7: Lucide icons as rune::Icon
src/theme.rs           # Task 7: tokens, Kit, MarkdownStyle
src/ui/mod.rs          # Task 8: root view
src/ui/titlebar.rs     # Task 8
src/ui/menus.rs        # Task 8
src/ui/sidebar_view.rs # Task 10
src/ui/document.rs     # Task 10
src/ui/empty.rs        # Task 10
src/ui/keys.rs         # Task 11
src/*_tests.rs / tests in sibling files per house rule
fixtures/              # small .md files used by tests and scenes
```

---

### Task 1: Crate scaffold and path helpers

**Files:**
- Create: `Cargo.toml`, `src/main.rs` (temporary hello window), `src/paths.rs`, `src/paths_tests.rs`, `.gitignore` (`/target`)

**Interfaces:**
- Produces:
  - `pub fn is_markdown(p: &Path) -> bool`
  - `pub fn canonical(p: &Path) -> PathBuf`. Uses `std::fs::canonicalize`; on error it falls back to an absolute path made by joining `std::env::current_dir()`.
  - `pub fn display_titles(paths: &[PathBuf]) -> Vec<String>`. Returns the file name; when two or more share a file name, each gets `"{name} — {parent_dir_name}"`.

- [ ] **Step 1:** `git init`. Write `Cargo.toml`:
  - package `marko`, `edition = "2024"`;
  - dependencies exactly as in Global Constraints, plus `serde = { version = "1", features = ["derive"] }`, `serde_json = "1"`, `ignore = "0.4"`;
  - dev-dependency `tempfile = "3"`;
  - `[profile.release] lto = "thin"`, `strip = true`.
- [ ] **Step 2: Write the failing tests** in `src/paths_tests.rs`:
  - `is_markdown_accepts_listed_extensions_any_case`: `a.md`, `B.MARKDOWN`, `c.Mdown`, `d.mkd` and `e.txt` are true; `f.png`, `g` and `.md` (no stem) are false.
  - `canonical_resolves_dot_segments_and_symlinks`: in a tempdir, write `a.md` and symlink `link.md → a.md` (unix only, `#[cfg(unix)]`). `canonical(dir/"./a.md") == canonical(dir/"link.md")`.
  - `display_titles_disambiguates_duplicates`: input `[/x/tables/README.md, /x/rune/README.md, /x/rune/PLAN.md]` gives `["README.md — tables", "README.md — rune", "PLAN.md"]`.
- [ ] **Step 3:** Run `cargo test paths` and expect failures (functions missing).
- [ ] **Step 4:** Implement the three functions in `src/paths.rs`. `main.rs` holds only the hello window for now: copy `examples/hello` from the Rune repo, change the title to `"Markdown"`.
- [ ] **Step 5:** `cargo test paths` passes. `cargo run` opens a window.
- [ ] **Step 6:** Commit `scaffold: crate, path helpers`.

### Task 2: Loading a document

**Files:** Create `src/load.rs`, `src/load_tests.rs`, `fixtures/basic.md` (headings, a table, a ```rust block, a ```mermaid flowchart, a task list), `fixtures/frontmatter.md`.

**Interfaces:**
- Produces:
  ```rust
  pub enum DocState {
      Loading,
      Ready { src: std::sync::Arc<str>, large: bool, lossy: bool },
      Error(LoadError),
  }
  pub enum LoadError { NotFound, Permission, Binary, TooLarge { bytes: u64 }, Io(String) }
  pub fn load(path: &Path) -> DocState                // blocking; called only from workers
  pub fn strip_frontmatter(src: &str) -> &str          // D11 fallback for upstream R2
  ```
  Also `impl LoadError { pub fn message(&self) -> String }` with this exact copy:
  - Binary: `"This isn't a text file"`
  - TooLarge: `"This file is too large to open (over 50 MB)"`
  - NotFound: `"This file no longer exists"`
  - Permission: `"You don't have permission to read this file"`
  - Io(e): `e`

- [ ] **Step 1: Failing tests:**
  - `load_reads_utf8`;
  - `load_flags_lossy_for_invalid_utf8` (bytes `b"ok \xff\xfe"` → `lossy: true`, `src` contains `"ok"`);
  - `load_refuses_binary` (a NUL at byte 10 → `Error(Binary)`);
  - `load_flags_large_over_5mb` (5 MB + 1 byte of `a` → `large: true`);
  - `load_refuses_over_50mb`. Create the file sparse with `set_len(50*1024*1024+1)`; it must return `TooLarge` **without reading** the content (check the size with `metadata` first).
  - `load_missing_is_not_found`;
  - `strip_frontmatter_yaml_and_toml`: `"---\na: 1\n---\n# T"` → `"# T"`, `"+++\na=1\n+++\n# T"` → `"# T"`, `"# T\n---\n"` unchanged, `"---\nunterminated"` unchanged.
- [ ] **Step 2:** `cargo test load` fails.
- [ ] **Step 3:** Implement. The frontmatter block must start on line 1 and close with a line that is exactly `---` or `+++`, matching the opener.
- [ ] **Step 4:** `cargo test load` passes.
- [ ] **Step 5:** Commit `load: read documents with size, binary and utf-8 checks`.

### Task 3: TabSet

**Files:** Create `src/tabs.rs`, `src/tabs_tests.rs`.

**Interfaces:**
- Consumes: `paths::canonical`, `paths::display_titles`, `load::DocState`.
- Produces:
  ```rust
  pub struct Tab { pub id: u64, pub path: PathBuf, pub doc: DocState, pub scroll: f32 }
  pub struct TabSet { /* tabs: Vec<Tab>, active: Option<usize>, closed: Vec<PathBuf> (max 20), next_id: u64 */ }
  impl TabSet {
      pub fn new() -> Self;
      pub fn open(&mut self, path: &Path) -> (u64, bool);   // (tab id, newly_created); focuses existing by canonical path
      pub fn close(&mut self, id: u64);
      pub fn close_others(&mut self, id: u64);
      pub fn close_to_right(&mut self, id: u64);
      pub fn reopen_closed(&mut self) -> Option<u64>;
      pub fn activate(&mut self, id: u64);
      pub fn activate_index(&mut self, i: usize);           // ⌘1…⌘9; 9 = last tab
      pub fn cycle(&mut self, forward: bool);
      pub fn move_tab(&mut self, id: u64, to: usize);
      pub fn active(&self) -> Option<&Tab>;
      pub fn active_mut(&mut self) -> Option<&mut Tab>;
      pub fn get_mut(&mut self, id: u64) -> Option<&mut Tab>;
      pub fn iter(&self) -> impl Iterator<Item = &Tab>;
      pub fn titles(&self) -> Vec<String>;                  // display_titles over tab paths, in tab order
      pub fn is_empty(&self) -> bool;
  }
  ```
  New tabs start with `DocState::Loading`. They are inserted **directly after the active tab** (at the end when none is active) and become active.

- [ ] **Step 1: Failing tests:**
  - `open_inserts_after_active_and_activates`;
  - `open_dedupes_by_canonical_path` (Review Focus 1: `dir/a.md`, `dir/./a.md` and a symlink give one tab, and the second open returns `newly_created == false` with that tab active);
  - `close_active_picks_right_then_left` (Review Focus 4: tabs A, B, C with B active; close B → C active; close C → A active; close A → `active() == None`, `is_empty()`);
  - `reopen_closed_restores_last_closed_path`;
  - `activate_index_nine_is_last`;
  - `cycle_wraps_both_ways`;
  - `close_others_and_close_to_right`;
  - `move_tab_reorders_and_keeps_active_id`.
- [ ] **Step 2:** `cargo test tabs` fails.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test tabs` passes.
- [ ] **Step 5:** Commit `tabs: TabSet with dedupe, close order, reopen`.

### Task 4: Sidebar roots and folder scan

**Files:** Create `src/sidebar.rs`, `src/sidebar_tests.rs`.

**Interfaces:**
- Consumes: `paths::{canonical, is_markdown}`.
- Produces:
  ```rust
  pub const SCAN_CAP: usize = 20_000;
  pub struct Root { pub path: PathBuf, pub name: String, pub tree: rune_kit::TreeModel<rune_kit::FileNode<PathBuf>>, pub scanning: bool, pub error: Option<String> }
  pub struct Sidebar { pub roots: Vec<Root>, pub visible: bool }
  pub enum AddRoot { Added(usize), AlreadyRoot(usize), InsideRoot { root: usize } }
  impl Sidebar {
      pub fn add_root(&mut self, dir: &Path) -> AddRoot;   // canonical compare; new roots start scanning=true, empty tree
      pub fn remove_root(&mut self, index: usize);
      pub fn root_of(&self, file: &Path) -> Option<usize>; // which root contains this canonical path
  }
  pub fn scan(dir: &Path) -> Result<Vec<PathBuf>, String>;          // blocking, worker only; Markdown files relative to dir, sorted
  pub fn build_tree(root_key: &str, files: &[PathBuf], base: &Path) -> rune_kit::TreeModel<rune_kit::FileNode<PathBuf>>;
  pub fn readme_or_first(files: &[PathBuf]) -> Option<PathBuf>;      // README.md any case at top level, else first file
  ```
  - `scan` uses `ignore::WalkBuilder::new(dir).hidden(true).git_ignore(true)` with a `filter_entry` that drops `node_modules`, `target` and `.venv`. It keeps only `is_markdown` files and stops after `SCAN_CAP`.
  - `build_tree` uses `FileTree::new().key(root_key).dirs_first(true).build(...)`, where each item is `(relative path string with '/' separators, absolute PathBuf)`. See `rune-kit/src/tree/file_tree.rs:84`.

- [ ] **Step 1: Failing tests:**
  - `scan_finds_markdown_sorted`;
  - `scan_skips_ignored_and_caps` (Review Focus 5: a tempdir with `node_modules/x.md`, `.hidden/y.md`, `target/z.md`, a `.gitignore` listing `out/`, and `out/w.md` gives none of those; a separate test with a cap override shows the walk stops at the cap). To keep the test fast, add `fn scan_with_cap(dir, cap)` and have `scan` call it with `SCAN_CAP`.
  - `add_root_detects_duplicates_and_nesting`;
  - `root_of_matches_canonical_paths`;
  - `readme_or_first_prefers_readme_any_case`;
  - `build_tree_nests_folders_dirs_first`: assert `tree.children(None)` order puts folder ids before file ids.
- [ ] **Step 2:** `cargo test sidebar` fails.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test sidebar` passes.
- [ ] **Step 5:** Commit `sidebar: roots, ignored-aware scan, tree build`.

### Task 5: AppState and the open model

**Files:** Create `src/app.rs`, `src/open.rs`, `src/open_tests.rs`.

**Interfaces:**
- Consumes: Tasks 1–4.
- Produces:
  ```rust
  // app.rs
  pub enum Mode { Preview }                     // Edit arrives in phase 2
  pub struct Toast { pub text: String, pub until: f64 }
  pub struct AppState {
      pub tabs: TabSet, pub sidebar: Sidebar, pub recent: Vec<PathBuf>,
      pub zoom: f32, pub wide: bool, pub appearance: Appearance,  // Appearance { System, Light, Dark }
      pub drop_hover: bool, pub menu: Option<MenuKind>,            // MenuKind { Open, More }
      pub toast: Option<Toast>, pub jobs: Vec<Job>,                // work for the worker, drained by main.rs
      pub drag: rune_kit::shell::TabDragState,
  }
  pub enum Job { Load { tab: u64, path: PathBuf }, Scan { root: PathBuf } }
  impl AppState {
      pub fn new() -> Self;                                       // AppSpec::init
      pub fn loaded(&mut self, tab: u64, doc: DocState);           // worker result
      pub fn scanned(&mut self, root: &Path, files: Result<Vec<PathBuf>, String>);
      pub fn show_toast(&mut self, text: impl Into<String>, now: f64);  // 2.4 s
      pub fn remember_recent(&mut self, path: &Path);             // front, dedupe, cap 16
  }
  // open.rs
  pub struct Classified { pub files: Vec<PathBuf>, pub folders: Vec<PathBuf>, pub rejected: usize }
  pub fn classify(paths: &[PathBuf]) -> Classified;   // stat each path; dirs → folders; is_markdown → files; else rejected
  impl AppState { pub fn open_paths(&mut self, paths: &[PathBuf], now: f64); }
  ```
  `open_paths` rules (spec §5.1):
  - For each file: `tabs.open`, and push `Job::Load` when the tab is newly created. Then `remember_recent`.
  - For each folder: `sidebar.add_root`. On `Added`, push `Job::Scan` and `remember_recent`. On `InsideRoot`/`AlreadyRoot`, do nothing in this task.
  - When `rejected > 0`, show the toast `"Skipped {n} file that isn't Markdown"` or `"Skipped {n} files that aren't Markdown"`.
  - When a scan finishes (`scanned`) and `tabs.is_empty()`, open `readme_or_first`.
  - On a scan `Err`, set `root.error` (the view shows `"Can't read this folder"`). An empty file list makes the view show `"No Markdown files"`.

- [ ] **Step 1: Failing tests:**
  - `mixed_drop_routes_each_kind` (Review Focus 3: tempdir folder + `a.md` + `b.png` gives 1 root, 1 tab, the toast text `"Skipped 1 file that isn't Markdown"`, and jobs `[Load, Scan]` in that order);
  - `opening_same_file_twice_queues_one_load`;
  - `recent_is_front_dedupe_capped_16`;
  - `scan_result_opens_readme_when_no_tabs`;
  - `scan_result_does_not_open_when_tabs_exist`;
  - `scan_error_sets_root_error`;
  - `toast_expires_after_2_4_seconds`. Add `pub fn tick(&mut self, now: f64) -> bool` returning true when the toast was cleared; it is used by `poll`.
- [ ] **Step 2:** `cargo test open` fails.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** `cargo test` passes in full.
- [ ] **Step 5:** Commit `open: route files to tabs and folders to sidebar`.

### Task 6: Session, Recent and the instance lock

**Files:** Create `src/session.rs`, `src/session_tests.rs`.

**Interfaces:**
- Consumes: `AppState`, `rune::prefs::Prefs` (`Prefs::open(id)`, `Prefs::at(path)` for tests, `get`/`set`; `rune/src/prefs.rs:63-100`).
- Produces:
  ```rust
  #[derive(Serialize, Deserialize, Default)]
  pub struct Session { pub tabs: Vec<PathBuf>, pub active: Option<usize>, pub roots: Vec<PathBuf>,
                       pub sidebar_visible: bool, pub zoom: f32, pub wide: bool, pub appearance: String,
                       pub recent: Vec<PathBuf>, pub frame: Option<(i32, i32, u32, u32)> }
  pub fn capture(s: &AppState) -> Session;
  pub fn restore(s: &mut AppState, session: Session, now: f64);   // goes through open_paths; skips missing paths
  pub fn load(prefs: &Prefs) -> Session;                          // key "session"; Default when absent/corrupt
  pub fn save(prefs: &mut Prefs, session: &Session);
  pub struct InstanceLock(std::fs::File);
  pub fn primary_lock(dir: &Path) -> Option<InstanceLock>;        // std::fs::File::try_lock on dir/"instance.lock"; None if held
  pub struct SaveDebounce { /* last_change: Option<f64> */ }
  impl SaveDebounce { pub fn touch(&mut self, now: f64); pub fn due(&mut self, now: f64) -> bool; } // due once, ≥1.0 s after last touch
  ```
  - Restore order: roots first, then tabs, then `active`. CLI paths are added **after** restore (spec §5.1).
  - Only a process holding `primary_lock` ever calls `save`.

- [ ] **Step 1: Failing tests:**
  - `capture_then_restore_round_trips` (via `Prefs::at(tempdir/prefs.json)`);
  - `restore_skips_missing_paths` (Review Focus 2: the session lists a deleted file and a deleted folder; restore gives no tab, no root, no panic, and `recent` no longer contains them);
  - `corrupt_session_falls_back_to_default`;
  - `second_lock_in_same_dir_is_refused`;
  - `debounce_fires_once_after_one_second_quiet`.
- [ ] **Step 2:** `cargo test session` fails.
- [ ] **Step 3:** Implement. `File::try_lock` needs Rust ≥ 1.89. Put `rust-version = "1.89"` in `Cargo.toml`.
- [ ] **Step 4:** `cargo test` passes.
- [ ] **Step 5:** Commit `session: persist tabs, roots, prefs; single saver`.

### Task 7: Icons and theme

**Files:** Create `src/icons.rs`, `src/theme.rs`, `src/theme_tests.rs`.

**Interfaces:**
- Produces:
  - `icons.rs`: `pub static PANEL_LEFT, FOLDER_OPEN, FOLDER, FILE_TEXT, FILE, FILE_PLUS, SEARCH, ELLIPSIS, X, EYE, PENCIL, CHEVRON_RIGHT, CHEVRON_DOWN, COPY, LINK, ROTATE_CW, ZOOM_IN, ZOOM_OUT, MOVE_HORIZONTAL, SUN, TRASH_2, DOWNLOAD: rune::Icon`.
    - Each is `Icon { name: "lucide-<name>", view: 24.0, body: r#"…"# }`.
    - Copy the SVG bodies from `ui-preview.html`, and change `stroke`/`fill` to `stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"` on each element. Glyph format reference: `rune-kit/src/glyphs.rs`.
    - Add `pub static ALL: &[&rune::Icon]`.
    - Put the Lucide ISC licence text in the module doc comment.
  - `theme.rs`:
    - `pub struct Tokens { bar, side, line, line_strong, fg, fg2, fg3, hover, press, accent, accent_soft, code_bg, code_line: rune::Color }`;
    - `pub fn tokens(dark: bool) -> &'static Tokens`, with values exactly from spec §7;
    - `pub fn kit(dark: bool) -> &'static rune_kit::Kit` via `OnceLock` + `Kit::from_palette(PaletteSeed { bg, fg, accent })`. Seeds: light `#FFFFFF`/`#1C1917`/`#2F6BFF`, dark `#18181B`/`#EDEDEF`/`#6B93FF`. Pattern: `examples/kit-gallery/src/main.rs:85-101`.
    - `pub fn markdown_style(dark: bool, zoom: f32) -> rune_kit::MarkdownStyle`, starting from `MarkdownStyle::from_kit(kit(dark))`. Apply the §4.7 sizes multiplied by `zoom` wherever `MarkdownStyle` exposes a builder (read `rune-kit/src/markdown/style.rs`). List any §4.7 value it cannot express in a `// TODO(upstream):` comment. Don't fake it.
    - `pub const BAR_H: f32 = 40.0; BTN: f32 = 28.0; ICON: f32 = 16.0; SIDE_W: f32 = 248.0; ROW_H: f32 = 28.0; READ_W: f32 = 720.0; READ_W_WIDE: f32 = 960.0;`
- [ ] **Step 1: Failing tests:**
  - `icon_names_unique_and_bodies_use_current_color` (iterate `ALL`);
  - `tokens_match_spec_samples` (light `accent == #2F6BFF`, dark `bar == #1F1F23`);
  - `kit_is_cached` (same pointer twice).
- [ ] **Step 2:** `cargo test theme` fails.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Tests pass.
- [ ] **Step 5:** Commit `theme: tokens, kit, markdown style, lucide icons`.

### Task 8: Titlebar and menus

**Files:** Create `src/ui/mod.rs`, `src/ui/titlebar.rs`, `src/ui/menus.rs`, `src/ui/view_tests.rs`. Modify `src/main.rs` so `AppSpec` uses `AppState::new` and `ui::view`.

**Interfaces:**
- Consumes: `AppState`, `theme::*`, `icons::*`.
- Produces:
  - `pub fn view(s: &AppState, cx: &mut Cx<AppState>) -> El`. It returns `rune_kit::root(base, layers)`, where `base` is a column of titlebar, a 1pt `line` divider, and the body (Task 10 fills the body; here a placeholder `el().key("body")`).
  - Element keys used by tests and by later tasks (exact):

  | Key | Element |
  |---|---|
  | `tb-sidebar` | toggle; flips `sidebar.visible` |
  | `tb-open` | opens the Open menu (`menu = Some(Open)`; pressing again closes it) |
  | `tabs` | the `TabStrip`, keyed `"tabs"` |
  | `mode-preview` | the active segment |
  | `mode-edit` | disabled; pressing it does nothing |
  | `tb-find` | **disabled in 1a**; the tooltip says `"Find arrives soon"` |
  | `tb-more` | opens the More menu |
  | `menu-open-file`, `menu-open-folder`, `menu-new` (disabled), `menu-recent-{i}`, `menu-clear-recent` | Open menu items |
  | `menu-close-tab`, `menu-copy-path`, `menu-zoom-in`, `menu-zoom-out`, `menu-zoom-reset`, `menu-wide`, `menu-appearance-{system,light,dark}` | More menu items |

  - The titlebar is `WindowTitlebar::from_insets(cx.chrome_insets(), 1.0).height(theme::BAR_H).leading(…).tabs(strip).trailing(…).build(kit)` (`rune-kit/src/shell/titlebar.rs`).
  - The strip is `TabStrip::new("tabs", TabOrientation::Horizontal)`. Wire it as follows (`rune-kit/src/shell/tab_strip.rs:296-483`):
    - `.tabs(...)`: `TabItem::new(id_string, title).icon(&icons::FILE_TEXT)`;
    - `.active(...)`;
    - `.drag_state(&s.drag, |s| &mut s.drag)`;
    - `.on_select(...)` → `tabs.activate`;
    - `.on_close(...)` → `tabs.close`;
    - `.on_reorder(...)` → `tabs.move_tab`.
  - Tab ids in the strip are `tab.id.to_string()`.
  - The menus are popovers anchored under their buttons, built from rune-kit `menu.rs` / `popover.rs`. Measurements and items in order come from spec §4.4 and §4.5. Show shortcut labels with `KeyEvent`-style text: `⌘O` on macOS, `Ctrl+O` elsewhere (`cfg!(target_os = "macos")`).
  - `menu-open-file` calls `tx.open_file(FileDialogOptions { title: "Open Markdown".into(), ..Default::default() }.filter("Markdown", &["md","markdown","mdown","mkd","txt"]), |s, r, tx| s.dialog_answer(r, tx.now()))`. Check the real `Tx::open_file` signature at `rune/src/file_dialog.rs:205`, the `FileDialogResult` shape, and how to read the current time in a handler (if `Tx` has no clock, keep `now` in `AppState`, updated in `poll`). `menu-open-folder` uses `tx.open_folder`. Add `AppState::dialog_answer(&mut self, r: FileDialogResult, now: f64)`, which calls `open_paths` on `Ok(Some(p))`, ignores `Ok(None)`, and toasts `"Couldn't open the file dialog"` on `Err`.
- [ ] **Step 1: Failing view tests** (`rune::Runtime::new(AppState::new(), ui::view, (1100.0, 760.0), Env { system_dark: false, scale: 2.0 })`; pattern in Rune `examples/hello/src/main.rs` tests):
  - `sidebar_button_toggles_visibility`;
  - `open_button_opens_and_closes_menu`;
  - `edit_segment_is_inert`;
  - `tab_strip_selects_and_closes`. Seed the state with two tabs via `open_paths` on fixtures, press the strip tab's element, assert `active` changed, then press its close.
  - `dialog_answer_opens_file` (unit-level: call `dialog_answer(Ok(Some(fixture)))` directly; Runtime has no public scripted-dialog hook).
- [ ] **Step 2:** `cargo test ui` fails.
- [ ] **Step 3:** Implement. A test helper `press(rt, key)` does `rt.find(key)` + `pointer_down`/`pointer_up` + a few `rt.frame(1.0/60.0)`. TabStrip tab keys: read the TabStrip source for the per-tab key format and use it; don't guess.
- [ ] **Step 4:** Tests pass. `cargo run` shows the titlebar; on Linux it sits under the native title bar (expected, D11).
- [ ] **Step 5:** Commit `ui: titlebar, tab strip, open and more menus`.

### Task 9: main wiring, workers, CLI, session I/O

**Files:** Modify `src/main.rs`. Create `src/worker.rs`.

**Interfaces:**
- Consumes: `AppState.jobs`, `load::load`, `sidebar::scan`, `session::*`.
- Produces: `pub fn spawn_worker(handle: rune::AppHandle<AppState>) -> std::sync::mpsc::Sender<Job>`. It runs one thread that executes jobs in order and posts results with `handle.update(|s, _| s.loaded(..) / s.scanned(..))`. Check `AppHandle`'s real generic shape at `rune/src/app.rs:708`.

`main` order:
1. On Linux, `std::env::remove_var("WAYLAND_DISPLAY")` (an `unsafe` block in edition 2024, with a `// SAFETY: single-threaded, before any thread starts` comment).
2. `rune::prewarm_gpu()`.
3. Build `AppSpec { min_size: Some((560.,400.)), on_key: Some(ui::keys::on_key), poll: Some(poll), scenes: SCENES, ..AppSpec::new("Markdown", (1100.,760.), AppState::new, ui::view) }`.
4. `App::new(spec).titlebar(Titlebar::new(40.0))`. On macOS, also `.standard_menus()`.
5. Get a `handle`, then in `.on_ready(...)`:
   - spawn the worker;
   - take the `primary_lock` from the Prefs dir (`Prefs::open("dev.marko.app").path()`'s parent);
   - `session::restore`, then `open_paths(cli_paths)`, where `cli_paths` are `std::env::args_os().skip(1)` filtered with `rune::extra_args()` excluded. Read `rune/src/tooling.rs:130` for which args Rune consumes.
6. `.run()`.

`poll(s, now, tx)`:
- drain `s.jobs` into the worker sender (keep the sender in a `OnceLock` static, since `poll` is a fn pointer);
- `s.tick(now)`;
- when anything changed, `SaveDebounce::touch`;
- when `due` and this process is primary, `session::save`.
- Return true **only** when state visible on screen changed.

`SCENES`: `("empty", …)`, `("folder", …)` (opens `fixtures/` as a root plus `fixtures/basic.md`), `("files", …)`, `("menu-open", …)`, `("drop", |s| s.drop_hover = true)`. They are used with `cargo run -- --snap out.png --scene folder` (Rune's tooling; see `examples/hello`).

- [ ] **Step 1: Failing test** in `src/worker.rs` tests: `worker_runs_load_and_posts_result`. If `AppHandle` can't be built in a test, test a `fn run_job(job: Job) -> JobResult` split out from the thread loop instead, and state that choice in the commit message.
- [ ] **Step 2:** Implement main, poll and worker.
- [ ] **Step 3: Verify:**
  - `cargo test` is all green.
  - `cargo run -- fixtures/basic.md fixtures` opens with one tab and one sidebar root.
  - Quit and run `cargo run` again: the same tab and root come back.
  - With the app open, a second `cargo run` opens a window, and after both quit, the first one's session is intact.
- [ ] **Step 4:** Commit `main: workers, CLI paths, session restore and save`.

### Task 10: Body (sidebar, document, empty state, drop, errors)

**Files:** Create `src/ui/sidebar_view.rs`, `src/ui/document.rs`, `src/ui/empty.rs`. Modify `src/ui/mod.rs`. Extend `src/ui/view_tests.rs`.

**Interfaces:**
- Consumes: everything above.
- Produces these element keys:
  - `side`, `side-root-{i}`, `side-root-remove-{i}`, `side-loose` (the "Open files" section);
  - `doc` (scroll container), `doc-error`, `doc-banner-large`, `doc-banner-lossy`;
  - `empty`, `empty-open-file`, `empty-open-folder`, `empty-recent-{i}`;
  - `drop-overlay`.

Rules (spec §4.6, §4.7, §4.9, §4.10, §4.12):
- **Body:** `row(sidebar if visible && !tabs.is_empty() || !roots.is_empty(), document-or-empty)`.
- **Sidebar:**
  - 248pt wide, `side` fill, right border `line`;
  - the "Open files" section lists tabs whose `sidebar.root_of(path)` is None; it shows above the roots and only when non-empty;
  - each root shows a header (uppercase name, 11pt semibold `fg3`, hover `x` → `remove_root`) and then a `Tree::new(...).row(|n, r| TreeItem::file(n, r)).on_select(|s, id, tx| open that file)` (`rune-kit/src/tree.rs` module doc).
  - Root status rows: `"Scanning…"` while scanning, `"No Markdown files"`, or `"Can't read this folder"`.
  - Selecting a tab calls `root.tree.reveal(id, tx)` for its file.
- **Document:**
  - `Markdown::new(strip_frontmatter(src)).style(markdown_style(dark, zoom)).scroll_wide(true).selectable(true).tasks_read_only().on_link(route_link).build(kit)` inside a vertical scroll;
  - centred column, max width `READ_W` (or `READ_W_WIDE`), padding 40 / 40 / 96;
  - `route_link`: `http:`, `https:` or `mailto:` → `tx.open_url`; a relative path ending in a Markdown extension → resolve against the doc's dir and `open_paths`; anything else → ignore in 1a.
  - Check whether rune-kit's `on_link` gives you a `Tx`; if it doesn't, use `on_link_tx` (`markdown/links.rs`).
  - `Loading` → nothing (one frame). `Error(e)` → centred `file` icon, `"Can't open {name}"`, and `e.message()` in `fg3`.
  - `large` → banner `"Large file — some features are slower"`. `lossy` → banner `"Some characters couldn't be read"`.
- **Empty state:** exactly per §4.9 and `ui-preview.html` (Empty). Recent shows at most 5 entries.
- **Drop:**
  - on the root element: `.on_drop_files(|s, paths, tx| s.open_paths(paths, now))`;
  - `.on_drop_hover(|s, on, _| s.drop_hover = on)`;
  - `.accepts_files(|p| p.is_dir() || paths::is_markdown(p))`;
  - `.on_drop_rejected(...)` counts the rejected files into the same toast as `open_paths`.
  - Check in `rune/src/el/drop.rs` whether rejected and accepted files of one drop arrive in separate callbacks, and make a mixed drop show **one** toast.
  - `drop-overlay` renders only while `drop_hover`.
- **Toast:** a top-centre pill, keyed `toast`, from `s.toast` (spec §4.11).
- [ ] **Step 1: Failing view tests:**
  - `empty_state_when_no_tabs`;
  - `closing_last_tab_shows_empty_state` (Review Focus 4);
  - `drop_mixed_paths` (Review Focus 3: `rt.drop_files(vec![dir, md, png], None)`, then frames, gives a root, a tab and the toast key `toast` present);
  - `drop_hover_shows_overlay`;
  - `error_doc_shows_message` (a tab whose doc is `Error(Binary)` shows `doc-error`);
  - `loose_files_section_only_for_files_outside_roots`.
- [ ] **Step 2:** Run the tests and expect failures.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Tests pass.
- [ ] **Step 5: Visual check.**
  - Run `cargo run -- --snap target/folder.png --scene folder` and the same for `empty` and `drop`, in both light and dark (see Rune's `--snap` options for appearance).
  - Compare each snap side by side with `ui-preview.html` in the matching state.
  - Fix spacing and colour drift against the §7 tokens.
  - Commit the stills under `stills/`.
- [ ] **Step 6:** Commit `ui: sidebar, document, empty state, drop, errors`.

### Task 11: Keyboard map

**Files:** Create `src/ui/keys.rs`, `src/ui/keys_tests.rs`.

**Interfaces:**
- Produces `pub fn on_key(s: &mut AppState, ev: &rune::KeyEvent, tx: &mut rune::Tx) -> bool`.
- Use `ev.is_command('o')` and friends (`rune/src/edit/keys.rs:116-140`), which already map ⌘ on macOS to Ctrl elsewhere. Check `ev.mods.shift` for ⇧ chords.
- Bindings from spec §4.13, **excluding Find (1b)**:

| Key | Action |
|---|---|
| O | open file (same handler as `menu-open-file`) |
| ⇧O | open folder |
| B | toggle sidebar |
| W | close tab |
| ⇧T | reopen closed |
| Ctrl+Tab / Ctrl+Shift+Tab | cycle |
| 1–9 | `activate_index` |
| R | reload (re-queue `Job::Load` for the active tab) |
| + / − / 0 | zoom: steps 0.1, clamp 0.6–2.0, 0 resets to 1.0 |
| ⇧C | copy the active path with `tx.copy` |
| Escape | closes an open menu |

- Return true only when handled.
- [ ] **Step 1: Failing tests:**
  - `cmd_w_closes_active_tab`;
  - `cmd_b_toggles_sidebar`;
  - `cmd_9_activates_last`;
  - `ctrl_tab_cycles`;
  - `zoom_clamps`;
  - `escape_closes_menu`.
  - Build the `KeyEvent` values the way Rune's own tests do; grep `KeyEvent {` in `crates/rune/src`.
- [ ] **Step 2:** Run the tests and expect failures.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Tests pass.
- [ ] **Step 5:** Commit `keys: keyboard map`.

### Task 12: Phase 1a exit check

- [ ] `cargo test` is green, and `cargo clippy --all-targets -- -D warnings` is clean.
- [ ] Each source file is under 800 lines (`wc -l src/**/*.rs`, test files excluded).
- [ ] Manual check on the machine at hand:
  - every mockup state is reachable: empty, folder, loose files, menus open, drop hover, and an error tab;
  - light and dark both work;
  - drag-and-drop works on Linux (X11 via the `WAYLAND_DISPLAY` removal);
  - an idle window draws nothing: watch CPU at 0% for 30 s with a doc open.
- [ ] Write `NOTES-1a.md`: what deviated from the spec, every `TODO(upstream)`, and every Rune API that behaved differently than this plan says. The next plan (1b: find, images, live reload, code copy) starts from it.
- [ ] Commit `phase 1a: exit notes`.
