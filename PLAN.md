# Rune Markdown — plan

A small, fast desktop Markdown app built on Rune. Phase 1 is a **viewer**; phase 2 adds **editing** on the same renderer.

- UI mockup: `ui-preview.html` (open it in a browser; switch platform, state, theme, and label the parts).
- Rune source of truth: the Rune repo on `dev`. Every API named here was checked against it on 2026-11-10; file:line refs point into `crates/`.
- This document is for the engineer building it. Sections marked **Decided** came from the product owner. Sections marked **Builder's call** are defaults you may change if you write down why.

---

## 1. Why and what

You can open a Markdown file, a folder of them, or drop either onto the window. You get a fast, native preview that renders everything a README or design doc uses: GFM, highlighted code, tables and Mermaid diagrams. The app should start instantly, use no CPU when idle, and look like a well-made native app on macOS, Windows and Linux.

It is also a showcase app for Rune, so wherever the gap is generic, fix it in Rune or rune-kit rather than working around it in the app (see §9).

### Goals (phase 1)
1. Open files and folders three ways: the Open menu, the OS dialogs, and drag-and-drop. Opening adds to what is already open and never replaces it.
2. Render CommonMark plus GFM: tables, task lists, strikethrough, autolinks, highlighted code, Mermaid, images and links. §8 lists everything that must render.
3. **One custom titlebar with every control in it**, on macOS, Windows **and Linux**. There is no toolbar and no status bar.
4. Live reload: when an open file changes on disk, the tab updates on its own.
5. Light: fast cold start, zero idle CPU, small memory footprint (§10 has the budgets).

### Non-goals (phase 1)
- Editing, creating or saving files. Phase 2 covers that; §12 outlines it.
- An outline or "On this page" panel. **Decided: removed.**
- Export (PDF, HTML), printing, plugins, sync, web or iOS builds.
- Multi-window. One window holds many tabs.

### Anti-goals (the app fails even if it "works" when…)
- It draws frames while idle, or the fans spin with a doc open.
- The titlebar looks like a web page on Linux, or sits under a second system title bar.
- Something in a real README renders as raw text, such as frontmatter, `> [!NOTE]` or a footnote.
- Opening something you dropped closes or replaces what you already had open.

---

## 2. Decisions so far

| # | Decision | By |
|---|---|---|
| D1 | Built on Rune + rune-kit, pinned to a Rune `dev` rev (never a branch). | Owner |
| D2 | All controls in the titlebar. No toolbar, no status bar. | Owner |
| D3 | Open a file, open a folder, or drag-and-drop either. Each addition adds tabs or sidebar roots. | Owner |
| D4 | Phase 1 is preview only. The Edit segment shows but is disabled. | Owner |
| D5 | No `+` in the titlebar. "New Markdown File ⌘N" sits in the Open menu, greyed out until phase 2. | Owner |
| D6 | The outline panel is removed. | Owner |
| D7 | The custom titlebar must work on Linux too, the way Tauri's `decorations: false` does. | Owner |
| D8 | Icons are Lucide (ISC), vendored as `rune::Icon` statics. | Builder's call (matches mockup) |
| D9 | The app lives in its own repo and depends on Rune by git rev. | Builder's call (Rune apps pin `dev` revs) |
| D10 | Linux v1 runs on the X11 backend (XWayland on Wayland desktops) so drag-and-drop works from day one. Native Wayland drops come later. | Owner |
| D11 | Someone else reviews and merges Rune / rune-kit PRs, so app work must not stall on them (§9.1). | Owner |

---

## 3. Platform and titlebar

### 3.1 What Rune does today
- `App::titlebar(Titlebar::new(40.0))` (`rune/src/app.rs:173`, `rune/src/titlebar.rs`) gives the app a self-drawn titlebar band:
  - **macOS:** full-size content view; the traffic lights stay native and are placed with `Titlebar::traffic_lights(x, y)`.
  - **Windows:** no native title bar. Rune draws the three 46pt caption buttons over the trailing edge, and Snap Layouts, Aero Snap and resize edges keep working.
  - **Linux: the native frame stays.** `Cx::chrome_insets()` returns zero and the band sits *under* the system title bar, which breaks D7.
- `El::window_drag()` (`rune/src/el/layout.rs:278`) marks drag regions. Double-click toggles maximize (`window/chrome.rs:72`).
- `Tx::toggle_maximized()` (`el/tx.rs:224`), `Tx::minimize()` (`el/tx.rs:218`) and `Tx::close_window()` (`el/tx.rs:203`).
- `WindowOptions::borderless` (`rune/src/windows.rs:74`) removes the system frame, but on every OS it also:
  - turns minimize into a no-op (`window/shell_requests.rs:20`);
  - leaves no edge resize, because winit's `drag_resize_window` is never called;
  - makes Rune ignore `App::titlebar` (`window.rs:509`).
- rune-kit already has the band layout: `WindowTitlebar::from_insets(insets, zoom).leading(..).tabs(strip).trailing(..)` (`rune-kit/src/shell/titlebar.rs`).

### 3.2 Phase 0: make `Titlebar` client-side on Linux (a Rune PR, before app work)
Tauri does the same thing on Linux: it turns off decorations and the web page draws the buttons. The Rune fix keeps the API the same on every platform: **`App::titlebar` on Linux turns the window into a client-side-decorated window.**

1. On Linux (X11 and Wayland), when a titlebar is set, create the window with `with_decorations(false)`.
2. Draw caption buttons at the trailing edge with the Windows code path (`titlebar/caption.rs`), restyled for GNOME and KDE: 24pt round buttons, 12pt apart, 12pt from the edge (mockup → Linux). `ChromeInsets` reports their width so the app keeps clear of them.
   - Optional: read the button order and side from `org.gnome.desktop.wm.preferences button-layout` over the existing `dbus` dependency. Default: minimize, maximize, close on the right.
3. Edge resize: hit-test a 6pt invisible border (8pt at corners) and call `window.drag_resize_window(ResizeDirection::…)` on press. Set the matching resize cursors on hover.
4. Allow `Tx::minimize()` for these windows. The guard at `shell_requests.rs:20` should block only true borderless overlays and panels, not titlebar windows.
5. Right-click on a drag region shows the window menu: `window.show_window_menu(pos)`, which works on Wayland and is a no-op elsewhere.
6. Shadow and corners:
   - **X11:** draw a 1px `--line-strong` border. A compositor shadow isn't guaranteed.
   - **Wayland:** without server-side decorations there is no shadow. Leave 0 corner radius in v1. Rounded corners and a shadow need a transparent surface and an inset margin; track that as a follow-up, not a blocker.
7. Tests:
   - headless `ChromeInsets` on Linux;
   - caption hit-tests;
   - minimize is allowed for a titlebar window and still refused for an overlay.
8. Add a changelog bullet, `changelog/unreleased/<date>-linux-client-titlebar.md`, because this changes Linux app output.

**Acceptance:** on Ubuntu 24.04 (GNOME, Wayland and X11 sessions) and on KDE Plasma 6:
- the window shows only the app's titlebar;
- drag, double-click maximize, minimize, maximize, close and all 8 resize edges work;
- Super+arrow snapping works.

### 3.3 Linux drag-and-drop on Wayland: known gap
winit 0.30 delivers **no file drops on Wayland** (`rune/src/el/drop.rs:61`). GNOME and KDE both default to Wayland, so D3 fails there out of the box. Options, in order of preference:
1. **Phase 0b (Rune):** add Wayland `wl_data_device` file drops to Rune's winit fork or patch set, or upstream to winit. This is the real fix and the largest item in phase 0.
2. **Interim:** let the app choose the X11 backend on Linux, so it runs under XWayland where drops work, behind a Rune `RunOptions`-style builder flag such as `prefer_x11()`. The cost is blurrier fractional scaling under XWayland.
3. Until either lands, the empty state's Open buttons and the Open menu remain the path on Wayland. The drop overlay simply never appears.

**Decided (D10):** ship v1 with option 2 as the Linux default, and land option 1 when it's ready. Phase 0 therefore includes only the small X11-preferred flag. Native Wayland drops are a later Rune PR, and when it lands, Wayland becomes the default again.

---

## 4. UI specification
The mockup is the reference. Where it and this text disagree, this text wins.

### 4.1 Window
- Default size 1100×760pt, minimum 560×400pt. Remember the frame across launches; §6.6 covers where.
- Layout from top to bottom: the **titlebar (40pt)**, then the **body**, which holds the sidebar (optional) and the document.

### 4.2 Titlebar: left to right
| Zone | Contents | Notes |
|---|---|---|
| Window controls (macOS) | Native traffic lights | `Titlebar::new(40.).traffic_lights(16., 14.)`. Reserve 76pt. |
| Leading | **Sidebar toggle** (`panel-left`, ⌘B), **Open** (`folder-open`, menu, ⌘O) | 28×28pt icon buttons, 16pt icons, 4pt apart. |
| Divider | 1×16pt `--line-strong`, 4pt margins | |
| Tabs | One tab per open file | rune-kit `TabStrip` (`shell/tab_strip.rs`). §4.3 has the details. |
| Drag region | Flexible, at least 32pt | `.window_drag()`. Double-click maximizes. On Linux and Windows, right-click opens the window menu. |
| Mode | Segmented **Preview \| Edit** with `eye` and `pencil` icons | rune-kit `SegmentedControl`. Edit is disabled with the tooltip "Edit arrives in phase 2". |
| Divider | as above | |
| Trailing | **Find** (`search`, ⌘F), **More** (`ellipsis`, menu) | |
| Window controls (Windows and Linux) | Minimize, maximize, close | Rune draws them (§3.2). Keep clear of `Cx::chrome_insets()`. |

Every titlebar control:
- has a tooltip naming the action and its shortcut;
- shows hover with `--hover` and pressed with `--press`;
- draws a 2pt `--accent` focus ring for keyboard focus.

Buttons inside the drag region keep their presses; Rune handles this.

Shortcut labels follow the platform. macOS shows `⌘O`; Windows and Linux show `Ctrl+O`. The mockup switches them per platform.

### 4.3 Tabs
- 28pt tall, 6pt radius, 8pt leading pad, 4pt trailing pad. Width is 96–200pt and shrinks evenly.
  - When tabs still overflow at 96pt, the strip scrolls horizontally, and a chevron at the end opens a list of all tabs.
- Contents: a `file-text` icon (14pt; `--accent` when active), the file name in 12.5pt medium with ellipsis in the middle for long names, and a close `x` (20pt hit area). The `x` is visible on hover and on the active tab.
- The active tab has a `--surface` fill, a 1pt `--line` border and a faint shadow. Inactive tabs are transparent with `--fg-2` text.
- A 6pt `--accent` dot means the file changed on disk while the tab was in the background. Activating the tab clears it.
- Hovering a tab shows a tooltip with the full path.
- When two tabs share a name, both show the parent folder, for example `README.md — tables`.
- Reorder by drag (`TabDragState`). No tear-out in phase 1.
- Middle-click closes a tab. ⌘W closes the active tab, Ctrl+Tab and Ctrl+Shift+Tab cycle, and ⌘1…⌘9 jump to a tab.
- Closing the last tab shows the empty state.
- Right-click menu: Close, Close Others, Close to the Right, Copy Path, Show in Folder.

### 4.4 Open menu (popover from the Open button)
1. New Markdown File, disabled, with a "Phase 2" pill.
2. —
3. Open File… ⌘O. The dialog filters `md, markdown, mdown, mkd, txt` and allows multiple selection.
4. Open Folder… ⌘⇧O.
5. — "Recent" header — up to 8 entries, each with an icon (file or folder), a name, and the parent path in `--fg-3`. A missing entry shows struck through and is removed when clicked.
6. —
7. Clear Recent.

Menu rows are 30pt tall with 8pt padding, 8pt between icon and label, and the shortcut right-aligned in `--fg-3`. Hover is `--accent` with `--on-accent` text. The popover has a 4pt inner pad, an 8pt radius, `--shadow-pop`, and a 140ms enter (fade plus 4pt rise). It enters without motion when Reduce Motion is on.

### 4.5 More menu
Reload from Disk ⌘R · Show in Finder / Explorer / Files · Copy Path ⌘⇧C · Copy Markdown Source · — · Zoom In ⌘+ · Zoom Out ⌘− · Actual Size ⌘0 · Wide Reading Column (toggle) · — · Appearance ▸ System / Light / Dark · — · Close Tab ⌘W.

### 4.6 Sidebar
- 248pt wide, `--side` background, 1pt `--line` on the right. ⌘B toggles it.
- Phase 1 width is fixed. A drag-to-resize splitter (180–400pt) is optional; `SplitView` exists.
- **Folders open:** one section per root folder, in the order they were added.
  - Each section header is the folder name, 11pt semibold uppercase `--fg-3`, 28pt tall, with a hover `x` that removes the root.
  - Under each root is a `rune-kit` `Tree` built with `FileTree` (`rune-kit/src/tree/file_tree.rs`). It is virtualized and supports `reveal(path)`.
  - Rows are 28pt with 16pt indent per level, a 14pt chevron, a 16pt icon, 6pt gaps and a 6pt radius. The selected row uses an `--accent-soft` fill.
  - Show only folders that contain Markdown somewhere below them, plus Markdown files. Folders sort first, then case-insensitive natural order.
  - Ignore `.git`, `node_modules`, `target`, `.venv`, dotfolders and anything in `.gitignore` (use the `ignore` crate).
- **Loose files:** an "Open files" section lists every tab whose file is outside all roots. With no roots it is the only section. With roots it sits **above** them and disappears when it would be empty. Revealing such a tab selects its row in this section. *(Builder's call.)*
- Clicking a file opens it in a new tab, or focuses the tab that already shows it. Selecting a tab reveals its file in the tree.
- Keyboard: arrows, Enter, and Left/Right to collapse and expand (`Tree` gives these).
- An empty or unreadable folder shows one muted row: "No Markdown files" or "Can't read this folder".

### 4.7 Document
- Reading column: max 720pt of text with 40pt side padding, centered. "Wide" raises it to 960pt. Top padding is 40pt, bottom padding 96pt.
- Type scale (zoom multiplies all of these):

| Element | Size / line height | Weight | Spacing |
|---|---|---|---|
| Body | 16 / 1.7 | 400 | 16 after paragraphs |
| Lead (first paragraph under H1, optional) | 18 / 1.6 | 400, `--fg-2` | |
| H1 | 32 / 1.2 | 700, −2% tracking | 16 after |
| H2 | 22 / 1.3 | 650, −1% tracking | 40 before, 12 after, 1pt `--line` underline 8 below |
| H3 | 18 / 1.4 | 650 | 32 before, 8 after |
| H4–H6 | 16 / 1.5 | 650 (H6 `--fg-2`) | 24 before |
| Inline code | 0.875em mono | | `--code-bg` fill, 1pt `--code-line`, 4pt radius, 2/5 padding |
| Code block | 13 / 1.65 mono | | 8pt radius, 32pt header row (language label + copy button), 12/16 padding |
| Table | 14 / 1.5 | header 13 / 600 `--fg-2` | cells 8/12 padding, 8pt-radius outer border, header fill `--code-bg` |
| Blockquote | body | `--fg-2` | 3pt `--line-strong` left bar, 16pt pad |

- Use `Markdown::new(src).style(style).scroll_wide(true)…build(&kit)` (`rune-kit/src/markdown.rs:208`).
- `scroll_wide(true)` gives wide tables and code their own horizontal scroll, so the page never scrolls sideways.
- Text is selectable (`.selectable(true)`). Copying a selection copies plain text, and **Copy Markdown Source** copies the whole file.
- Each tab keeps its own scroll position, restored when you switch back.

### 4.8 Find (⌘F)
- A floating bar 8pt below the titlebar and 16pt from the right edge:
  - 36pt tall, 8pt radius, `--shadow-pop`;
  - search icon, a 200pt input, a "3 of 12" count, previous and next buttons, close.
- Enter goes to the next match, Shift+Enter to the previous, and Esc closes the bar and clears the highlights.
- Uses `rune_kit::markdown::find(src, query, Case::Insensitive)` (`markdown/find.rs:48`) → `.highlights(..)`. Scroll to the current match with `Markdown::highlight_key(i)`.
- All matches get a yellow wash and the current match a stronger orange (light and dark tokens in §7).
- Find applies to the active tab and runs again when you switch tabs or the file reloads.

### 4.9 Empty state (no tabs)
- Centered, 440pt max width:
  - a 56pt rounded-square badge with a `file-text` icon in `--accent` on `--accent-soft`;
  - "Open some Markdown" in 17pt semibold;
  - "Drop files or a folder anywhere on this window." in `--fg-2`;
  - a primary **Open File…** button and a secondary **Open Folder…** button, 32pt tall with their shortcuts;
  - a Recent card with up to 5 rows (icon, name, parent in `--fg-3`, relative time on the right).
- Find is disabled here, and the sidebar is hidden.

### 4.10 Drop overlay
- Shows while a drag carrying acceptable paths hovers the window: `on_drop_hover` on the root `El`.
- It is a full-window 70% `--bg` scrim with a 3pt blur, a 2pt dashed `--accent` inset 12pt from the edges, and a centered card: download icon, "Drop to open", and "Files open as tabs · folders join the sidebar".
- Accept `.md/.markdown/.mdown/.mkd/.txt` files and any directory, via `accepts_files(|p| p.is_dir() || is_markdown(p))` (`el/drop.rs:70`).
- If anything is rejected, show a toast: "Skipped 2 files that aren't Markdown" (`on_drop_rejected`).

### 4.11 Toasts
Use rune-kit `ToastRegion`, top center, 12pt under the titlebar. Each toast is a 30pt pill in inverted colors with 12pt medium text, lasts 2.4s, and at most one shows at a time. Use toasts for:
- a reload that changed visible content;
- skipped files after a drop;
- a deleted file;
- a read error.

### 4.12 Error and edge states (in the document area)
| Case | Behaviour |
|---|---|
| File deleted while open | Keep the last content, dim it to 60%, and show an inline banner at the top of the column: "This file was deleted · Close Tab". If it comes back, reload and remove the banner. |
| Unreadable / permission | Centered message with a `file-x` icon, "Can't open README.md", the OS error in `--fg-3`, and a **Try Again** button. |
| Not UTF-8 | Decode lossily and show a banner: "Some characters couldn't be read". |
| Binary file (NUL in first 8KB) | Refuse with the message "This isn't a text file". |
| Very large (> 5 MB) | Open, with a banner: "Large file — find and live reload are slower". There is no hard cap below 50 MB; above that, refuse. |
| Same path opened twice | Focus the existing tab and don't duplicate it. Compare canonical paths. |

### 4.13 Keyboard map
| Action | macOS | Windows / Linux |
|---|---|---|
| Open file / folder | ⌘O / ⌘⇧O | Ctrl+O / Ctrl+Shift+O |
| Toggle sidebar | ⌘B | Ctrl+B |
| Find / next / previous | ⌘F / ⌘G or Enter / ⇧⌘G or Shift+Enter | Ctrl+F / F3 or Enter / Shift+F3 |
| Close tab / reopen closed tab | ⌘W / ⌘⇧T | Ctrl+W / Ctrl+Shift+T |
| Next / previous tab | ⌃Tab / ⌃⇧Tab, ⌘⇧] / ⌘⇧[ | Ctrl+Tab / Ctrl+Shift+Tab |
| Jump to tab | ⌘1…⌘9 | Ctrl+1…9 |
| Reload | ⌘R | Ctrl+R / F5 |
| Zoom | ⌘+ / ⌘− / ⌘0 | Ctrl+ / Ctrl− / Ctrl+0 (and Ctrl+scroll) |
| Copy path | ⌘⇧C | Ctrl+Shift+C |
| Scroll | Space / ⇧Space, Home/End, ↑↓ | same |

Handle these in `AppSpec::on_key` (`tooling.rs:17`). `App::global_shortcut` registers OS-wide hotkeys, so don't use it here. On macOS also install `App::standard_menus()` so Quit, Hide and the Edit menu behave natively; Rune's menu items can carry the same shortcuts.

---

## 5. Behaviour

### 5.1 Opening model (D3)
- **Files**, from the dialog, a drop, the CLI or Recent: each opens in a new tab, appended after the active tab, and the last one becomes active. Paths already open are focused instead.
- **Folders**: each becomes a new sidebar root, unless it is already a root or inside one, in which case reveal it. If no tab is open, open the folder's `README.md` (any case) or, failing that, its first Markdown file.
- **A mix in one drop**: folders go to the sidebar and files to tabs.
- **CLI**: `rune-markdown [paths…]`. Directories are treated as folders and files as files. The last session (§5.4) is always restored first, and any paths given are then added, following the same rule as D3. With no arguments, only the session is restored. *(Builder's call.)*
- **macOS "Open With" and Finder double-click**: Rune has no open-documents event today (`on_open_url` covers URL schemes only). File it as a Rune PR in phase 1c: an `App::on_open_files` handler wired to `application:openURLs:` with file URLs. Until it lands, CLI and drop are the paths in.
- **Second launch**: `rune::single_instance(id)` only works on Windows. Phase 1 opens a second window/process; forwarding paths to the first instance is phase 1c, a small socket under the user runtime dir.
  - Until forwarding lands, only the process that holds an exclusive lock file in the config dir saves the session. A second process runs normally but never writes the session, so the two can't overwrite each other's tabs. Recent files are still merged on write. *(Builder's call.)*

### 5.2 Links
- `Markdown::on_link` (`markdown/links.rs:15`):
  - `http(s):` and `mailto:` → `Tx::open_url` (`el/tx.rs:255`);
  - relative `*.md` paths (optionally with `#anchor`) → open or focus that file as a tab;
  - `#anchor` → scroll to the heading (needs heading anchors, §9 R5);
  - other relative files → open with the OS default app.
- Hovering a link shows its target (`link_help`).
- Never auto-load remote content without a click.

### 5.3 Images
- `Markdown::images(|src| …)` (`markdown/image.rs:45`) receives the raw `src`. Resolve relative paths against the document's folder, decode off-thread (png, jpeg, gif first frame, webp, svg through Rune's `svg` feature), and cache.
- Bound the cache at 64 entries **and** 128 MB of *decoded* pixels (AGENTS.md rule). Downscale anything wider than 2× the column width on decode.
- **Remote images (`https://…`):** off by default and shown as a placeholder: "Remote image · Load". Allow per-document or globally in Appearance. Reason: privacy and lightness.

### 5.4 Session and prefs
- Use `rune::prefs::Prefs::open("dev.rune.markdown")` (`rune/src/prefs.rs:55`). It stores JSON in the user config dir:
  - open tabs (paths + scroll offsets) and the active index;
  - sidebar roots plus expanded folders;
  - sidebar visible; zoom; wide column; appearance; recent (16 max, de-duplicated);
  - window frame.
- `rune-mac::remember_frame` is macOS-only, so store the frame in Prefs on every OS.
- Write at most once per second, after changes stop, and once on quit.

### 5.5 Live reload
- Use the `notify` crate (`RecommendedWatcher`, `PollWatcher` fallback on network filesystems). Watch:
  - each open file's **parent directory**, non-recursively, so atomic-rename saves from editors are caught;
  - each sidebar root, recursively, for tree updates.
- Debounce 150 ms per path, re-read on a worker thread, and compare a hash.
  - Push only real changes to the UI with `AppHandle::update(|s, tx| …)` (`app.rs:708`), using `tx.instant()` when the tab isn't visible.
  - Keep the scroll position. Anchor to the first visible block key (`b{i}`), clamped if the doc got shorter.
- Start watchers in `on_ready`, never before the first frame.

---

## 6. Architecture

### 6.1 Repository
```
rune-markdown/
  Cargo.toml          # rune, rune-kit (features: markdown, mermaid), notify, ignore, image
  src/
    main.rs           # args → App::new(spec).titlebar(..).on_ready(..).run()
    app.rs            # AppState, update entry points, on_key
    titlebar.rs       # the 40pt band (WindowTitlebar + TabStrip + buttons + menus)
    tabs.rs           # Tab, TabSet (open/focus/close/reorder/dedupe)
    sidebar/          # roots, FileTree wiring, scan worker, ignore rules
    doc/              # view of one doc: Markdown builder, style, links, images, find
    io/               # load (utf-8/binary/size checks), watcher, image cache
    prefs.rs          # session + settings (serde) on rune::prefs
    icons.rs          # Lucide → rune::Icon statics
    theme.rs          # tokens (§7) → Kit::from_palette + MarkdownStyle
  tests/              # headless view tests + fixture corpus (§11)
  fixtures/           # the Markdown corpus
```
Follow Rune's house rules in this repo too: files under 800 lines, one feature per module, and each feature owning its state.

### 6.2 State (sketch)
```rust
struct AppState {
    tabs: TabSet,            // Vec<Tab>, active: usize, closed: Vec<PathBuf> (for ⌘⇧T)
    sidebar: Sidebar,        // roots: Vec<Root { path, tree: TreeModel }>, visible: bool
    find: FindState,         // open, query, matches: Vec<Vec<Range>>, current
    ui: UiPrefs,             // zoom, wide, appearance
    drop_hover: bool,
    toasts: ToastQueue,
}
struct Tab { path: PathBuf, title: String, doc: DocState, scroll: f32, changed: bool }
enum DocState { Loading, Ready { src: Arc<str>, hash: u64 }, Deleted { last: Arc<str> }, Error(String) }
```
- `view` is a pure function of `AppState`. Only the active tab builds its `Markdown`.
- `rune-kit` caches parses (`parse_cached`, `markdown/cache.rs:43`), so switching tabs is cheap.

### 6.3 Threads
- UI thread: Rune.
- One worker pool (`std::thread` + channel is enough) for file reads, image decodes and folder scans. Results come back through `AppHandle::update`.
- The `notify` watcher thread posts debounced events to the same channel.
- Nothing blocking runs on the UI thread, including the initial file read. Show `Loading` for at most one frame, then content.

### 6.4 Theme
- Build `Kit::from_palette(PaletteSeed { … })` (`rune-kit/src/palette.rs:110`) from the §7 tokens for both appearances, and switch on the system appearance unless overridden.
- Markdown gets a `MarkdownStyle` built from the same tokens: type scale §4.7 and syntax colors §7.

### 6.5 Fonts
- UI and body: Inter, which Rune bundles on native (`text.rs:16`). It stays consistent across OSes and needs no extra binary weight.
- Code: the system mono (`Family::SystemMono` → SF Mono, Cascadia/Consolas, DejaVu/Ubuntu Mono).
- **Builder's call:** bundle JetBrains Mono (~250 KB, OFL) for cross-platform consistency. Measure the startup cost first.

---

## 7. Design system (tokens)
These are the exact values used in `ui-preview.html`.

**Spacing (4pt grid):** 4, 8, 12, 16, 20, 24, 32, 40.

**Sizes:**

| Token | Value |
|---|---|
| Titlebar | 40 |
| Icon button | 28 |
| Icon | 16, stroke 1.75 (14 for small) |
| Tab height | 28 |
| Sidebar width | 248 |
| Row height | 28 |
| Reading column | 720 (wide: 960) |
| Radii | s 6, m 8, l 12 |

**Type:**
- UI: 13pt regular and 12–12.5pt medium for controls; section headers 11pt semibold uppercase +4% tracking.
- Reading: §4.7.

**Colors (light / dark):**

| Token | Light | Dark |
|---|---|---|
| bg / surface | #FFFFFF | #18181B |
| bar | #F5F5F4 | #1F1F23 |
| side | #FAFAF9 | #1C1C20 |
| line / line-strong | #E7E5E4 / #D6D3D1 | #2C2C31 / #3A3A40 |
| fg / fg-2 / fg-3 | #1C1917 / #57534E / #A8A29E | #EDEDEF / #A1A1AA / #63636B |
| hover / press | fg @ 5.5% / 9% | white @ 6% / 10% |
| accent / accent-soft | #2F6BFF / 10% | #6B93FF / 14% |
| code-bg / code-line | #F7F7F6 / #ECEBEA | #1F1F23 / #2C2C31 |
| find match / current | #FFC800 @ 35% / #FF9600 @ 70% | same |
| syntax kw / str / fn / num / comment / type | #B4237A / #1A7F37 / #2F5FD0 / #A15C00 / #8C8781 / #7A3FC2 | #FF7AB8 / #7EE2A0 / #8FB2FF / #F5B86B / #73737D / #C7A1FF |

Check every text/background pair against WCAG AA (4.5:1 for body, 3:1 for ≥18pt and icons). `fg-3` is for decoration and metadata only, never essential text.

**Motion:**
- Menus: 140ms fade + 4pt rise.
- Toasts: 180ms.
- Tab activate: none (instant).
- Sidebar toggle: 200ms width spring through `rune-motion`.
- Everything respects Reduce Motion, and any motion change passes `scripts/motion-audit`.

**Icons (Lucide, vendored as `rune::Icon { name, view: 24., body }`):** `panel-left`, `folder-open`, `folder`, `file-text`, `file`, `file-plus`, `file-x`, `search`, `ellipsis`, `x`, `eye`, `pencil`, `chevron-right`, `chevron-down`, `chevron-up`, `copy`, `link`, `rotate-cw`, `zoom-in`, `zoom-out`, `move-horizontal`, `sun`, `trash-2`, `download`. Keep the Lucide licence in `icons.rs`.

---

## 8. Markdown coverage

| Feature | Rune today | Where to do the work |
|---|---|---|
| CommonMark (headings, lists, emphasis, quotes, rules, code) | ✅ | — |
| GFM tables, task lists, strikethrough, autolinks | ✅ (`parser.rs:435`, `.autolink`) | — |
| Task checkboxes | ✅ | `.tasks_read_only()` in phase 1 |
| Syntax highlighting | ✅ rust, js, ts, tsx, json, python, go, swift, sh, sql, html, css, yaml, diff | **R1** add c, cpp, c#, java, kotlin, ruby, php, toml, markdown, dockerfile, lua, zig. The `c_like` scanner covers several of these cheaply. |
| Code copy button | ❌ none built in | App: `.code_frame(|el, f| …)` adds the 32pt header with language and Copy (`Tx::copy`). Upstream if it turns out generic. |
| Mermaid | ✅ native, 17 diagram types (flowchart, sequence, state, class, ER, gantt, pie, mindmap, timeline, journey, git, quadrant, XY, sankey, block, requirement, C4) | Style the frame through `.diagram_frame`. |
| Images (local) | ⚠️ hook only | App: §5.3 |
| Links | ⚠️ hook only | App: §5.2 |
| YAML/TOML frontmatter | ❌ rendered as text | **R2** skip it in rune-kit (pulldown-cmark has `ENABLE_YAML_STYLE_METADATA_BLOCKS`). Optionally show it as a small collapsed "Properties" table. |
| Footnotes | ❌ | **R3** `ENABLE_FOOTNOTES`, plus rendering (superscript refs and a footnote list at the end) |
| GitHub alerts `> [!NOTE]` etc. | ❌ | **R4** `ENABLE_GFM` gives the blockquote kind. Render as callouts with icon and color. |
| Heading anchors / `#links` | ❌ (blocks keyed `b{i}`) | **R5** GitHub-style slugs → block keys, plus a scroll-to API |
| Math `$…$`, `$$…$$` | ❌ | **R6** *Phase 1c, behind a feature.* Lightest path is pulldown-cmark `ENABLE_MATH` and showing the source in a styled mono block, then real typesetting later. Don't pull in a JS engine. |
| Raw HTML blocks | ❌ shown as text | Phase 1: render the common inline tags (`<br>`, `<kbd>`, `<sub>`, `<sup>`, `<img>`, `<details>/<summary>`) and show others as muted source. No HTML engine. **R7** |
| Emoji shortcodes `:tada:` | ❌ | Low priority. A table-driven replace in the app. |
| Definition lists, wiki links, smart punctuation | ❌ | Out of scope for v1. |

R1–R7 are **rune-kit PRs**, each with fixture tests in `tests/markdown_fixture.rs` and goldens, plus a `changelog/unreleased` bullet. Order: R2, R4, R3, R5, R1, R7, R6.

---

## 9. Rune / rune-kit work list (upstream)
| ID | Change | Phase | Size |
|---|---|---|---|
| P0-a | Client-side `Titlebar` on Linux: caption buttons, edge resize, minimize allowed, window menu (§3.2) | 0 | M |
| P0-b | File drops on Wayland (§3.3, option 1). Interim: an X11-preferred flag. | 0 / 1 | L / S |
| P1-a | `App::on_open_files` for macOS open-documents events | 1c | S |
| P1-b | Single-instance forwarding on macOS and Linux | 1c | S–M |
| R1–R7 | Markdown coverage (§8) | 1b | S each |
| R8 | Optional: virtualize long documents. Today the whole doc is laid out; measure first (§10). | 1c if needed | M |

### 9.1 Working while upstream PRs wait for review (D11)
Rune PRs are reviewed by someone else, so the app never blocks on them:
- **Pinning:** the release build always pins a merged Rune `dev` rev. While a PR is in review, a development branch of the app may use a Cargo `[patch]` pointing at that PR's branch. Remove the patch before any release tag. CI fails a release tag whose `Cargo.lock` points anywhere other than the official Rune repo.
- **App-side fallbacks, until the matching PR merges:**

| PR | Fallback in the app |
|---|---|
| P0-a | Linux keeps the native frame, and the 40pt band sits under it (`ChromeInsets` zero). It works, but it's the degraded look. |
| P0-b flag | Remove `WAYLAND_DISPLAY` from the process environment at the top of `main`, before Rune starts, so winit picks X11. This is a one-liner and is removed when the flag lands. |
| R2 | Strip a leading `---…---` / `+++…+++` block from `src` before passing it to `Markdown::new`. |
| R4 | Leave as plain blockquotes. |
| R3, R5, R6, R7 | Leave unsupported. Release notes list them. |
| R1 | Unhighlighted mono code for the missing languages. |
| P1-a, P1-b | CLI and drop are the ways in; the lock-file rule in §5.1 covers second launches. |

- Remove each fallback in the same app PR that bumps Rune to the rev containing the fix.

Follow Rune's contribution rules: work on `dev` and fast-forward only, add a changelog bullet when output changes, respect the file-size caps, add builder methods instead of new pub fields on literal-built types, and keep private paths and names out of commits.

---

## 10. Lightness budgets
Measure with `--profile perf`. Compare paired alternating runs by median, and record what each number leaves out (GPU upload, presentation). Use `crates/procwatch` in-app for footprint and CPU.

| Metric | Budget | How measured |
|---|---|---|
| Cold start to first content frame (1 small file) | ≤ 250 ms (M-series Mac, mid-range Linux) | Rune flight log / timestamp in `on_ready` |
| Idle CPU, document open, no input | 0% (no frames drawn) | procwatch; Rune frame counter must stay flat |
| Memory footprint, 1 typical README | ≤ 90 MB | procwatch footprint |
| Memory, 20 tabs + 2 roots of ~2,000 files | ≤ 160 MB | procwatch |
| Tab switch (warm) | next frame (≤ 8 ms at 120 Hz) | frame-profile |
| Scroll a 10k-line doc | holds 120 Hz on a Mac with ProMotion | `markdown_resize`-style bench on a big fixture |
| Live reload of a 200 KB file | visible ≤ 100 ms after save | timestamp diff |
| Release binary (stripped, LTO) | ≤ 25 MB; report per OS | `ls -l` |

Rules:
- `poll` returns true only when something visible changed.
- The watcher and workers never request frames by themselves; they call `update`, and Rune redraws if the view changed.
- Follow "Launching fast" in `crates/rune/README.md`:
  - call `rune::prewarm_gpu()` first in `main`;
  - keep init and the first view cheap;
  - do slow work in `on_ready`;
  - land early state with `tx.instant()`;
  - run `scripts/app-audit` against the app.
- If a 10k-line fixture misses the scroll budget, do R8 before shipping.

---

## 11. Testing
- **Headless view tests** (Rune playground/testing, as `examples/showcase/src/tests.rs` does). Cover:
  - tab open, dedupe, close and reorder;
  - sidebar root add and remove;
  - drop accept and reject;
  - find highlight and navigation;
  - empty state;
  - each error state in §4.12.
- **Stills**: render light and dark of the empty, folder, loose-files, find-open, drop-hover and error states at 1100×760, and compare them to goldens.
- **Fixture corpus** `fixtures/`:
  - the GFM spec examples;
  - one file per §8 row;
  - real-world READMEs (Rune's own README, PLAN and CHANGELOG are good);
  - a 10k-line stress file;
  - a non-UTF-8 file, a binary file, and a 6 MB file.
- **Platform matrix (manual, each release):**
  - macOS 14+;
  - Windows 11 (light, dark, 125% and 150% scale);
  - Ubuntu 24.04 GNOME on Wayland and X11;
  - Fedora KDE 6 on Wayland.
  - For each: titlebar controls, resize edges, drag-and-drop, dialogs, live reload with VS Code and vim saves (atomic rename).
- **Watcher tests**: write, rename-over, delete then recreate, and a burst of 50 writes, which must debounce to one reload.

---

## 12. Phases and milestones

**Phase 0 — Rune prerequisites** (upstream PRs)
- P0-a, plus P0-b interim (the X11 flag). Acceptance is in §3.2.

**Phase 1a — Shell** (app)
- Window, titlebar band, tabs, Open menu with dialogs (`native-file-dialog` feature; `Tx::open_file` / `open_folder`, `file_dialog.rs:205`), drop, sidebar with `FileTree`, empty state, CLI args, Prefs session, keyboard map.
- Markdown renders through stock `Markdown::build`.
- *Exit:* every state in the mockup is reachable in the real app and matches it in both themes. Each OS exits on its own: macOS and Windows don't wait for phase 0, and Linux exits once P0-a lands. *(Builder's call.)*
- Linux reference machine for the budgets: a 4-core x86-64 laptop with integrated graphics (Intel Iris Xe class) on Ubuntu 24.04. Use an Apple M1 for macOS and the same laptop class for Windows. *(Builder's call; the owner can name their own.)*

**Phase 1b — Rendering completeness**
- Theme and `MarkdownStyle` from the tokens, code-frame copy button, link routing, local images with cache, find.
- R1–R5 upstream.
- *Exit:* the fixture corpus renders with no raw-syntax leaks, and stills are approved.

**Phase 1c — Polish and lightness**
- Live reload, error states, toasts, tab overflow and context menu, zoom, wide column, R7 (and R6 if wanted), P1-a, P1-b, budget pass (§10) with R8 if needed, packaging:
  - `.app` with a `.md` document-type claim;
  - MSIX or zip on Windows;
  - AppImage + `.desktop` with the MIME type on Linux.
- *Exit:* all budgets are met, and the manual matrix passes.

**Phase 2 — Edit** (separate plan when phase 1 ships)
- Enable the Edit segment. Edit mode is a split view: rune-kit `text_input` multi-line on the left and the same `Markdown` view on the right, with scroll sync by block key.
  - Phase 2 can also offer an editor-only layout.
- ⌘N creates "Untitled.md" in Edit mode, and ⌘S saves through `Tx::save_file`.
- A dirty dot on tabs, a confirm on closing with unsaved changes, and live reload that becomes "file changed on disk — Reload / Keep mine" when dirty.
- Decide then whether **New** also gets a titlebar button (open question from the owner).

---

## 13. Risks and open questions
| # | Item | Owner / default |
|---|---|---|
| Q1 | ~~Wayland drops~~ | **Decided (D10):** ship on X11, native Wayland drops later. |
| Q2 | Remote images off by default: OK? | Owner. Default off. |
| Q3 | Math in v1, and how much (source-styled vs typeset)? | Owner. Default: source-styled, behind a flag. |
| Q4 | App name, bundle id and icon. | Owner. Placeholder `rune-markdown`, `dev.rune.markdown`. |
| R-a | Wayland CSD has no shadow or rounded corners in v1, so it looks flatter than on macOS and Windows. | Accept for v1; follow-up in Rune. |
| R-b | Without virtualization, very long docs may miss 120 Hz. | Measure in 1c; R8. |
| R-c | `notify` misses events on some network filesystems. | Use the `PollWatcher` fallback with a 2s interval when the path is on NFS/SMB. |
| R-d | Inter for body text vs a reading serif. | Builder's call; Inter matches the mockup. |

---

## 14. Rune API map (quick reference)
| Need | API | Where |
|---|---|---|
| App + window | `App::new(AppSpec{…}).titlebar(Titlebar::new(40.)).on_ready(..).run()` | `rune/src/app.rs`, `tooling.rs:17` |
| Drag / insets | `El::window_drag()`, `Cx::chrome_insets()` | `el/layout.rs:278`, `runtime/cx.rs:138` |
| Window actions | `Tx::minimize`, `Tx::toggle_maximized`, `Tx::close_window`, `Tx::quit` | `el/tx.rs:203-224` |
| Titlebar band | `WindowTitlebar::from_insets(..).leading().tabs().trailing().build(&kit)` | `rune-kit/src/shell/titlebar.rs` |
| Tabs | `TabStrip`, `TabItem::new(id,title).icon().closable()`, `TabDragState` | `rune-kit/src/shell/tab_strip.rs` |
| Tree | `Tree`, `TreeModel`, `FileTree::new().build(paths)`, `model.reveal(path, tx)` | `rune-kit/src/tree*` |
| Menus / popovers / segmented / toast | `menu.rs`, `popover.rs`, `segmented.rs`, `toast.rs` | `rune-kit/src` |
| Markdown | `Markdown::new(src).style().scroll_wide().selectable().highlights().on_link().images().code_frame().diagram_frame().build(&kit)` | `rune-kit/src/markdown.rs:208` |
| Find | `markdown::find(src, q, Case)`, `Markdown::highlight_key(i)` | `markdown/find.rs:48` |
| Dialogs | `Tx::open_file`, `Tx::open_folder`, `FileDialogOptions{..}.filter("Markdown", &["md",…])`, feature `native-file-dialog` | `rune/src/file_dialog.rs:205` |
| Drop | `on_drop_files`, `on_drop_hover`, `on_drop_rejected`, `accepts_files` | `rune/src/el/drop.rs` |
| Open URL / clipboard | `Tx::open_url`, `Tx::copy` | `el/tx.rs:255`, `clipboard/tx.rs:19` |
| Prefs | `rune::prefs::Prefs::open(id)` | `rune/src/prefs.rs:55` |
| Cross-thread updates | `AppHandle::update(|s, tx| …)` | `app.rs:708` |
| Icons | `rune::Icon { name, view, body }`, `el::icon(&ICON, 16.)` | `text/icon.rs:14`, `el.rs:709` |
| Theme | `Kit::from_palette(PaletteSeed)` | `rune-kit/src/palette.rs:110` |
