//! The Edit view (§4.8): the Markdown source in a text area on the left, and the rendered
//! document beside it, updated as the source changes.

use rune::{El, ScrollSpec, TextEdit, col, el, row};
use rune_kit::{Kit, TextArea};

use crate::app::{AppState, Mode};
use crate::edit::EditBuffer;
use crate::load::DocState;
use crate::tabs::Tab;
use crate::theme;
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
    let tab = s.tabs.active_mut().expect("the editor is only shown for an active tab");
    &mut buffer(tab).text
}

/// True when the active tab's buffer differs from what was last saved or opened.
pub fn dirty(tab: &Tab) -> bool {
    tab.edit.as_ref().is_some_and(EditBuffer::dirty)
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
    s.mode = Mode::Edit;
    true
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

/// The split view for the active tab: the source on the left, its live preview on the right.
pub fn view(s: &AppState, k: &Kit, dark: bool) -> El {
    let t = theme::tokens(dark);
    let Some(tab) = s.tabs.active() else { return el().key("editor-split") };
    let fallback;
    let edit = match &tab.edit {
        Some(b) => &b.text,
        None => {
            fallback = TextEdit::new(source(&tab.doc));
            &fallback
        }
    };
    let source_area = TextArea::new(edit)
        .label("Markdown source")
        .width(None)
        .on_edit(edit_text)
        .build(k)
        .key("editor");
    let left = col().key("editor-pane").w_full().grow(1.0).min_h(0.0).px(PANE_PAD).py(PANE_PAD)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(source_area);
    let md = document::markdown(s, k, dark, &edit.text, &tab.path);
    let preview = document::reading("preview", s, md);
    row().key("editor-split").w_full().grow(1.0).min_h(0.0)
        .child(left)
        .child(el().w(1.0).h_full().fill(t.line))
        .child(preview)
}

#[cfg(test)]
#[path = "editor_tests.rs"]
mod tests;
