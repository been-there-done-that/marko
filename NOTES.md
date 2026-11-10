# Phase 1a notes

Read this before starting phase 1b. It lists what differs from `PLAN.md` and the Phase 1a plan, every `TODO(upstream)`, the Rune behaviour the plan got wrong, and what still needs a person to check.

## Deviations from the spec and plan

- **Icons.** Six icons from the Task 7 list are not used in 1a and were removed: `CHEVRON_RIGHT`, `CHEVRON_DOWN`, `LINK`, `ROTATE_CW`, `MOVE_HORIZONTAL`, `SUN`. `FILE_X` and `CHEVRON_UP` (not in the list) went too. Copy them from `ui-preview.html` when a menu needs them. `icons::ALL` is `#[cfg(test)]` and covers the icons that remain.
- **Theme.** `theme::BTN` was removed. `ROW_H` and `ICON` are now used by the sidebar and the empty state in place of literals.
- **Hover and press tokens are not applied.** `Tokens::bar`, `hover` and `press` (PLAN §7) are defined but nothing reads them, so the titlebar fill and the hover and press states come from Rune's defaults. Compare with the mockup on the manual pass.
- **Per-tab scroll is restored.** Each tab's document viewport has its own key, `doc-{id}`, and the offset is saved with the session (`progress.rs`). The reading-progress line and percentage pill follow the active document. Check on the manual pass that a reload and an Edit round trip keep the position.
- **Edit mode is inert.** `AppState::mode` is set but never read. The Preview | Edit control is hand-built and Edit has no press handler.
- **Tab context menu is phase 1b.** `TabSet::close_others`, `close_to_right` and their helper `retain_indices` are kept for it, behind `#[allow(dead_code)]` with that reason.
- **Reveal-on-select is phase 1b.** `AddRoot::AlreadyRoot` and `InsideRoot` carry a root index that nothing reads yet. Field-level allows say so.
- **Instance lock.** `InstanceLock` holds its `File` without reading it. The allow on the field says that dropping the file releases the lock.
- **Session frame.** `Session::frame` is always `None`. Window position and size are not saved.
- **Zoom** is clamped to 0.6 to 2.0 (`app.rs`).
- **Linux window frame.** As decided (D11), the native frame stays on Linux. The 40 pt band sits under the system title bar until the phase-0 Rune PR lands.
- **Scanned README counts as Recent.** When a folder scan finds nothing open, the README (or first file) opens as a tab, so it is added to Recent.
- **`rustfmt --check` fails** across the existing tree, mostly long lines. This pass did not reformat it. Add a `rustfmt.toml` and reformat in its own commit if wanted.
- **`#![allow(dead_code)]` is gone** from `main.rs`. Each remaining dead item carries a targeted allow with its reason.

## TODO(upstream)

All in `src/theme.rs`, in `markdown_style`. Each is a §4.7 value that `MarkdownStyle` cannot express yet:

1. Lead paragraph: 18 / 1.6, fg-2, first paragraph under the H1. No first-block style.
2. Vertical rhythm: 16 pt after paragraphs; H2 40 before and 12 after, H3 32 and 8, H4 to H6 24 before. No spacing builders.
3. H2's 1 pt `--line` underline 8 pt below the text. No rule-under-heading builder.
4. H6 in fg-2. `muted` covers levels 4 to 6, so H4 and H5 are fg too.
5. Table header at 13 pt (cells 14 pt). `table_font` sets both, so the header uses the cell size.
6. Table cell padding 8 / 12 and the 8 pt outer radius. No public padding builder.
7. Inline code's 4 pt radius and 2 / 5 padding. `inline_code_pad` takes one value.
8. Code block 8 pt radius and the 32 pt header row (language and copy). The header belongs to `code_frame` in the app.

## Rune behaviour that differs from the plan

- `MenuItem` has no custom key. The `menu-*` keys sit on the lead slot (`lead(...)`), and the tests press them there.
- `SegmentedControl` names its segment keys `segment{i}`, so the Preview | Edit control is hand-built.
- Tab close buttons all share the key `close`. Tests press the first match, which is the first tab's close button.
- `Tx` has no clock. `AppState::now` is set in `poll` each frame, and handlers read it from there.
- `on_ready` does not run in `--snap`, so anything started there is skipped in snapshots.
- `rune::extra_args()` already strips Rune's own flags. The app passes its remaining arguments through unchanged.
- `Tree` takes `fn` getters, so the sidebar is one virtual list. It has no Tree keyboard navigation, and arrow keys are not wired to it.
- `FileDialogOptions` has no multi-select in this rev. Open lets one file or folder through at a time.
- There is no quit hook. Changes made in the last second before quit may not be saved.
- Headless snaps default to dark. Pass `--theme light` for the light appearance.

## Not yet verified by a person

- Every mockup state is reachable in the real app (empty, folder, loose files, menus open, drop hover, error tab), and matches the mockup in light and dark.
- Drag-and-drop works on Linux under X11 (`WAYLAND_DISPLAY` removed at startup).
- Relaunch restores the saved session, and missing paths are dropped silently.
- Two instances: the second one does not save the session.
- Idle CPU: measured once on Linux (X11, debug build, `basic.md` + `fixtures/` open): 0 CPU ticks over 20 s, RSS 141 MB. The 90 MB budget still needs a release-build measurement.
- Native open file and open folder dialogs work.

# Phase 1b, 1c and 2 notes

Read this with the Phase 1a notes above. Where a 1a line says Edit mode is inert or a tab menu is missing, Phases 1b to 2 replace it.

## Phase 1b (tabs, find, guard, menus)

### Deviations

- **Tab menu closes through `AppState::close_clean`.** It skips tabs with unsaved edits, so a bulk close never loses work. `TabSet::close_others` and `close_to_right` are kept for the tests only.
- **⌘F focuses the find field via `TextEdit.focused`.**
- **Find highlights are not shown in the Edit preview.**
- **Show in Folder and Open other files spawn `xdg-open`, `open` or `explorer`**, because `tx.open_url` rejects `file:` URLs.

### TODO(upstream)

- `src/ui/document.rs`: remote images show their alt text, not the "Remote image · Load" placeholder. The Markdown image hook can only return an image or `None`.
- `src/theme.rs`, `markdown_style`: the §4.7 values `MarkdownStyle` cannot express yet (lead paragraph, vertical rhythm, heading rule, H6 colour, table header size and padding, inline-code radius and padding, code-block radius). The full list is under Phase 1a.

## Phase 1c (live reload, conflicts, save)

### Deviations

- **Remote images show alt text.** No HTTP client is in the app; see the TODO(upstream) above.
- **No SVG decode.** The image crate has no `svg` feature, so SVG images show their alt text.
- **Image cache is not invalidated when an image file changes.** Its byte bound also excludes mip levels, so the real footprint is somewhat larger than the bound.
- **Edit-preview images only load after a reload.**
- **Scroll sync is one way**: editor to preview. It is untested in a real window.
- **No PollWatcher fallback.** If the native watcher cannot start on a platform, live reload does nothing.
- **Watcher start errors are silent.** `Watcher::start` failures are dropped in `main.rs`.
- **Quitting with unsaved edits loses them.** Rune has no quit hook, so the guard only runs on tab close.

## Phase 2 (editor, packaging)

### Deviations

- **The editor uses the UI font.** `TextArea` has no mono font setter.
- **Packaging is Linux only.**

### TODO(upstream)

- `TextArea` needs a monospace font setter for the source pane.

### Unverified

- Drag-and-drop of files and folders onto the window (Linux, X11 with `WAYLAND_DISPLAY` removed).
- Native open and save dialogs.
- Relaunch restores the saved session, and missing paths are dropped silently.
- Two instances: the second one does not save the session.
- Typing feel in the editor.
- Scroll sync between editor and preview.
- Live reload in a real editor.

## Installed fonts (phase 3)

`fonts::load` asks fontconfig (`fc-list`) on Linux. Other systems return empty lists with `loaded: true`, so the pickers offer only the defaults there.
