//! The Edit view (§4.8): the Markdown source in a text area on the left, and the rendered
//! document beside it, updated as the source changes.

use rune::{El, ScrollPos, ScrollSpec, TextEdit, TextStyle, col, el, row, text, text_input};
use rune_kit::{Button, Kit};

use crate::app::{AppState, Mode};
use crate::edit::EditBuffer;
use crate::load::DocState;
use crate::settings::Settings;
use crate::tabs::Tab;
use crate::theme::{self, Look};
use crate::ui::document;

/// Padding around the source text area.
const PANE_PAD: f32 = 24.0;

/// The source a Ready document holds, or nothing for a tab that has none.
fn source(doc: &DocState) -> &str {
    match doc {
        DocState::Ready { src, .. } => src,
        _ => "",
    }
}

/// The tab's edit buffer, created from its loaded source the first time it is needed.
fn buffer(tab: &mut Tab) -> &mut EditBuffer {
    let doc = &tab.doc;
    tab.edit.get_or_insert_with(|| EditBuffer::from_src(source(doc)))
}

/// Edits from the source text area land in the active tab's buffer. The area is only in the
/// tree while the active tab is Ready, so there is always a tab to write to.
fn edit_text(s: &mut AppState) -> &mut TextEdit {
    let now = s.now;
    let tab = s.tabs.active_mut().expect("the editor is only shown for an active tab");
    tab.rev = tab.rev.wrapping_add(1);
    tab.last_edit = now;
    let buf = buffer(tab);
    buf.mark_edited();
    &mut buf.text
}

/// True when the active tab's buffer differs from what was last saved or opened. Hashes the
/// buffer, so per-frame code should use `dirty_hint` instead.
#[cfg_attr(not(test), allow(dead_code))]
pub fn dirty(tab: &Tab) -> bool {
    tab.edit.as_ref().is_some_and(EditBuffer::dirty)
}

/// The O(1) dirty hint for the tab strip, which asks about every tab every frame.
pub fn dirty_hint(tab: &Tab) -> bool {
    tab.edit.as_ref().is_some_and(EditBuffer::dirty_hint)
}

/// Whether the Edit view is shown this frame: Edit mode, with a Ready tab active.
pub fn shows(s: &AppState) -> bool {
    matches!(s.mode, Mode::Edit) && s.tabs.active().is_some_and(|t| matches!(t.doc, DocState::Ready { .. }))
}

/// Switches to Edit on the active tab, creating its buffer. Returns false when there is no
/// Ready tab to edit.
pub fn enter(s: &mut AppState) -> bool {
    let Some(tab) = s.tabs.active_mut() else { return false };
    if !matches!(tab.doc, DocState::Ready { .. }) {
        return false;
    }
    // Focus the source so the first key typed lands in it (the notes on `TextEdit::focused`).
    buffer(tab).text.focused = true;
    tab.preview_rev = tab.rev;
    s.mode = Mode::Edit;
    true
}

/// The notice for the active tab when its file changed on disk under unsaved edits. Reload drops
/// the edits and loads the file; Keep Mine keeps them, and the next save overwrites the file.
pub fn conflict_banner(s: &AppState, k: &Kit, look: &'static Look) -> Option<El> {
    if !s.tabs.active().is_some_and(|t| t.conflict) {
        return None;
    }
    let t = &look.tokens;
    let reload = Button::new("Reload")
        .on_press(|s: &mut AppState| s.reload_active_discarding())
        .build(k)
        .key("conflict-reload");
    let keep = Button::new("Keep Mine")
        .on_press(|s: &mut AppState| s.keep_mine_active())
        .build(k)
        .key("conflict-keep");
    Some(
        row().key("conflict-banner").w_full().mt(12.0).px(12.0).py(8.0).gap(8.0).radius(6.0)
            .fill(t.code_bg).border(1.0, t.code_line).center_y()
            .child(text("This file changed on disk").font(k.ty.small).color(t.fg2))
            .child(el().grow(1.0))
            .child(reload)
            .child(keep),
    )
}

/// Switches the active document to its rendered preview.
pub fn preview(s: &mut AppState) {
    s.mode = Mode::Preview;
}

/// ⌘E: Edit becomes Preview, and Preview becomes Edit when a tab can be edited.
pub fn toggle(s: &mut AppState) -> bool {
    if matches!(s.mode, Mode::Edit) {
        preview(s);
        return true;
    }
    enter(s)
}

/// The editing field's type: the code font at 13 points, scaled by zoom and the reader's text
/// size (16 is the default), with a 1.6 line height.
pub fn editor_style(look: &'static Look, settings: &Settings, zoom: f32) -> TextStyle {
    let size = 13.0 * zoom * settings.text_size / 16.0;
    TextStyle { size, line_height: size * 1.6, ..theme::code_font(&look.kit, settings) }
}

/// The split view for the active tab: the source on the left, its live preview on the right.
pub fn view(s: &AppState, k: &Kit, look: &'static Look) -> El {
    let t = &look.tokens;
    let Some(tab) = s.tabs.active() else { return el().key("editor-split") };
    let fallback;
    let edit = match &tab.edit {
        Some(b) => &b.text,
        None => {
            fallback = TextEdit::new(source(&tab.doc));
            &fallback
        }
    };
    let at = s.editor_at.clone();
    // rune-kit's `TextArea` draws in the UI font, so the field is built directly to use the code font.
    let ink = k.c.text_base;
    let source_area = text_input(edit, editor_style(look, &s.settings, s.zoom))
        .key("editor")
        .w_full()
        .color(ink)
        .caret(ink, k.selection())
        .label("Markdown source")
        .description("")
        .on_edit(edit_text)
        .animate_edit_layout();
    let left = col().key("editor-pane").w_full().grow(1.0).min_h(0.0).px(PANE_PAD).py(PANE_PAD)
        .scroll(ScrollSpec { stick_bottom: false })
        .on_scroll_offset(move |p: ScrollPos| {
            at.set([p.offset[1], p.max[1]]);
        })
        .child(source_area);
    let key = format!("edit-{}", tab.id);
    let md = document::markdown(
        s,
        k,
        look,
        document::MarkdownDoc { key: &key, rev: tab.preview_rev, src: &edit.text, path: &tab.path, highlights: Vec::new() },
    );
    let preview_at = s.preview_at.clone();
    let preview = document::reading("preview", s, md).on_scroll_offset(move |p: ScrollPos| {
        preview_at.set([p.offset[1], p.max[1]]);
    });
    row().key("editor-split").w_full().grow(1.0).min_h(0.0)
        .child(left)
        .child(el().w(1.0).h_full().fill(t.line))
        .child(preview)
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
