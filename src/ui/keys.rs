//! The keyboard map (§4.13): the app's shortcuts.

use rune::{Key, KeyEvent, Tx};

use crate::app::{AppState, Job};
use crate::find::{reveal_current, toggle as toggle_find};
use crate::load::DocState;
use crate::ui::menus::{close_tab, copy_path, open_file_dialog, open_folder_dialog};

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

fn dispatch(s: &mut AppState, ev: &KeyEvent, tx: &mut Tx) -> bool {
    if matches!(ev.key, Key::Escape) {
        if s.menu.take().is_some() {
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
    if chord(ev, 'f', false) {
        toggle_find(s, tx);
        return true;
    }
    if matches!(ev.key, Key::Tab) && ev.mods.ctrl {
        s.tabs.cycle(!ev.mods.shift);
        return true;
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
        s.sidebar.visible = !s.sidebar.visible;
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
