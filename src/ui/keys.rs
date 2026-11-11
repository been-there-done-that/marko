//! The keyboard map (§4.13): the app's shortcuts.

use std::path::Path;

use rune::{FileDialogOptions, Key, KeyEvent, Tx};

use crate::app::{AppState, Job};
use crate::find::{reveal_current, toggle as toggle_find};
use crate::load::DocState;
use crate::ui::editor;
use crate::ui::menus::{close_tab, copy_path, new_file, open_file_dialog, open_folder_dialog};

/// ⌘ (Ctrl off macOS) plus `c`, with ⇧ held exactly when `shift` says.
fn chord(ev: &KeyEvent, c: char, shift: bool) -> bool {
    ev.is_command(c) && ev.mods.shift == shift
}

/// Handles a key press. Returns true only when the key did something. A press that changes the
/// active tab also re-runs an open find, so its matches follow the tab.
pub fn on_key(s: &mut AppState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let before = s.tabs.active().map(|t| t.id);
    let handled = dispatch(s, ev, tx);
    if s.tabs.active().map(|t| t.id) != before {
        s.sync_find();
    }
    handled
}

/// Enter, Shift+Enter and Escape typed into the focused find field: the field would otherwise
/// keep them, so the window's `on_key` never sees them.
pub fn find_field_key(s: &mut AppState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    match ev.key {
        Key::Escape => {
            s.find.close();
            s.find.query.focused = false;
            true
        }
        Key::Enter if s.find.count() > 0 => {
            s.find_step(!ev.mods.shift);
            reveal_current(s, tx);
            true
        }
        Key::Enter => true,
        _ => false,
    }
}

/// Up, Down, Enter and Escape typed into the palette's field: the field keeps them, so the
/// window's `on_key` never sees them.
pub fn palette_field_key(s: &mut AppState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    let rows = crate::commands::ranked(s, &s.palette.query.text);
    match ev.key {
        Key::Escape => {
            s.palette.close();
            true
        }
        Key::Down => {
            s.palette.selected = (s.palette.selected + 1).min(rows.len().saturating_sub(1));
            true
        }
        Key::Up => {
            s.palette.selected = s.palette.selected.saturating_sub(1);
            true
        }
        Key::Enter => {
            if let Some(c) = rows.get(s.palette.selected) {
                crate::commands::run(s, &c.id, tx);
            }
            true
        }
        _ => false,
    }
}

/// Saves tab `id`. A titled tab writes straight to its path. An untitled tab, or Save As, first
/// asks the Save dialog where to write, and the tab takes the chosen path. `then_close` closes
/// the tab once the write lands (the guard's Save). Returns false when the tab is not loaded.
pub fn save_tab(s: &mut AppState, tx: &mut Tx, id: u64, save_as: bool, then_close: bool) -> bool {
    let Some(tab) = s.tabs.iter().find(|t| t.id == id) else { return false };
    if !matches!(tab.doc, DocState::Ready { .. }) {
        return false;
    }
    if !save_as && !tab.untitled {
        let path = tab.path.clone();
        s.queue_save(id, path, then_close);
        return true;
    }
    let file_name = tab.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let options = FileDialogOptions {
        title: "Save Markdown".into(),
        directory: tab.path.parent().map(Path::to_path_buf),
        file_name: Some(file_name),
        ..Default::default()
    }
    .filter("Markdown", &["md", "markdown"]);
    tx.save_file(options, move |s: &mut AppState, r, _: &mut Tx| match r {
        Ok(Some(path)) => {
            if let Some(t) = s.tabs.get_mut(id) {
                t.path = path.clone();
                t.untitled = false;
            }
            s.queue_save(id, path, then_close);
        }
        Ok(None) => {}
        Err(_) => {
            let now = s.now;
            s.show_toast("Couldn't open the file dialog", now);
        }
    });
    true
}

/// ⌘S saves the active tab; ⇧⌘S saves it under a new name.
fn save_active(s: &mut AppState, tx: &mut Tx, save_as: bool) -> bool {
    let Some(id) = s.tabs.active().map(|t| t.id) else { return false };
    save_tab(s, tx, id, save_as, false)
}

fn dispatch(s: &mut AppState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if matches!(ev.key, Key::Escape) {
        if s.lightbox.take().is_some() {
            return true;
        }
        if s.menu.take().is_some() {
            return true;
        }
        if s.palette.open {
            s.palette.close();
            return true;
        }
        if s.settings_open {
            s.settings_open = false;
            return true;
        }
        if s.find.open {
            s.find.close();
            return true;
        }
        return false;
    }
    if s.find.open && matches!(ev.key, Key::Enter) {
        if s.find.count() == 0 {
            return false;
        }
        s.find_step(!ev.mods.shift);
        reveal_current(s, tx);
        return true;
    }
    if chord(ev, 'p', true) || chord(ev, 'k', false) {
        if s.palette.open {
            s.palette.close();
        } else {
            s.palette.show();
        }
        return true;
    }
    if chord(ev, ',', false) {
        s.settings_open = !s.settings_open;
        s.menu = None;
        return true;
    }
    if chord(ev, 'f', false) {
        toggle_find(s, tx);
        return true;
    }
    if matches!(ev.key, Key::Tab) && ev.mods.ctrl {
        s.tabs.cycle(!ev.mods.shift);
        return true;
    }
    if matches!(ev.key, Key::PageUp | Key::PageDown) && ev.mods.ctrl && ev.mods.shift {
        return move_active_tab(s, matches!(ev.key, Key::PageDown));
    }
    if chord(ev, 'r', false) {
        return reload_active(s);
    }
    if chord(ev, 'o', false) {
        open_file_dialog(tx);
        return true;
    }
    if chord(ev, 'o', true) {
        open_folder_dialog(tx);
        return true;
    }
    if chord(ev, 'b', false) {
        s.toggle_sidebar(tx);
        return true;
    }
    if chord(ev, 'e', false) {
        return editor::toggle(s);
    }
    if chord(ev, 's', false) {
        return save_active(s, tx, false);
    }
    if chord(ev, 's', true) {
        return save_active(s, tx, true);
    }
    if chord(ev, 'n', false) {
        new_file(s);
        return true;
    }
    if chord(ev, 'w', false) && s.tabs.active().is_some() {
        close_tab(s);
        return true;
    }
    if chord(ev, 't', true) {
        return reopen_closed(s);
    }
    if chord(ev, 'c', true) && s.tabs.active().is_some() {
        copy_path(s, tx);
        return true;
    }
    if let Some(i) = ('1'..='9').position(|c| chord(ev, c, false)) {
        s.tabs.activate_index(i);
        return true;
    }
    if ev.is_command('+') || ev.is_command('=') {
        s.zoom_in();
        return true;
    }
    if ev.is_command('-') || ev.is_command('−') {
        s.zoom_out();
        return true;
    }
    if chord(ev, '0', false) {
        s.zoom_reset();
        return true;
    }
    false
}

/// Moves the active tab one place left or right in the strip. A tab at that end stays put, and
/// the press then does nothing.
fn move_active_tab(s: &mut AppState, forward: bool) -> bool {
    let Some(id) = s.tabs.active().map(|t| t.id) else { return false };
    let Some(at) = s.tabs.iter().position(|t| t.id == id) else { return false };
    let len = s.tabs.iter().count();
    let to = if forward { at.checked_add(1) } else { at.checked_sub(1) };
    let Some(to) = to.filter(|&i| i < len) else { return false };
    s.tabs.move_tab(id, to);
    true
}

/// Queues a fresh load for the active tab and shows it as loading again.
fn reload_active(s: &mut AppState) -> bool {
    let Some(tab) = s.tabs.active_mut() else { return false };
    tab.doc = DocState::Loading;
    let job = Job::Load { tab: tab.id, path: tab.path.clone() };
    if !s.jobs.contains(&job) {
        s.jobs.push(job);
    }
    true
}

/// Reopens the last closed tab. A tab that opened again while still loading gets its load
/// queued here, as `open_file` does for a new tab; a loaded one has nothing to fetch.
fn reopen_closed(s: &mut AppState) -> bool {
    let Some(id) = s.tabs.reopen_closed() else { return false };
    if let Some(tab) = s.tabs.get_mut(id)
        && matches!(tab.doc, DocState::Loading)
    {
        let job = Job::Load { tab: id, path: tab.path.clone() };
        if !s.jobs.contains(&job) {
            s.jobs.push(job);
        }
    }
    true
}

#[cfg(test)]
#[path = "keys_tests.rs"]
mod tests;
