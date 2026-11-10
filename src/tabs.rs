//! Open tabs: order, the active tab, closing, reopening, and dedupe by canonical path.

use std::path::{Path, PathBuf};

use std::sync::Arc;

use crate::edit::EditBuffer;
use crate::load::DocState;
use crate::paths::{canonical, display_titles};

/// How many closed paths `reopen_closed` can bring back.
const CLOSED_CAP: usize = 20;

pub struct Tab {
    pub id: u64,
    pub path: PathBuf,
    pub doc: DocState,
    #[allow(dead_code)] // Per-tab scroll memory is not restored yet (NOTES-1a).
    pub scroll: f32,
    /// Edit mode's buffer, set once the tab is first edited (Task 6 reads it).
    #[allow(dead_code)] // Read by Edit mode (Task 6).
    pub edit: Option<EditBuffer>,
    /// A New tab that has never been saved; its path does not exist on disk yet.
    #[allow(dead_code)] // Read by Save and the unsaved-changes guard (Task 7).
    pub untitled: bool,
}

pub struct TabSet {
    tabs: Vec<Tab>,
    active: Option<usize>,
    closed: Vec<PathBuf>,
    next_id: u64,
}

impl TabSet {
    pub fn new() -> Self {
        Self { tabs: Vec::new(), active: None, closed: Vec::new(), next_id: 1 }
    }

    /// Opens `path` as a tab and activates it. A file already open (by canonical
    /// path) is focused instead. Returns the tab id and whether a tab was created.
    /// A new tab goes directly after the active tab, or at the end when none is active.
    pub fn open(&mut self, path: &Path) -> (u64, bool) {
        let key = canonical(path);
        if let Some(i) = self.tabs.iter().position(|t| t.path == key) {
            self.active = Some(i);
            return (self.tabs[i].id, false);
        }
        (self.insert_tab(key, DocState::Loading, None, false), true)
    }

    /// Opens an empty, unsaved tab named `name` in the current directory and activates it.
    /// Its edit buffer starts empty, so it is not dirty until something is typed.
    #[allow(dead_code)] // The New command (Task 6/7) calls this.
    pub fn open_untitled(&mut self, name: &str) -> u64 {
        let dir = std::env::current_dir().unwrap_or_default();
        let doc = DocState::Ready { src: Arc::from(""), large: false, lossy: false };
        self.insert_tab(dir.join(name), doc, Some(EditBuffer::from_src("")), true)
    }

    /// Inserts a tab after the active one (or at the end) and activates it.
    fn insert_tab(&mut self, path: PathBuf, doc: DocState, edit: Option<EditBuffer>, untitled: bool) -> u64 {
        let at = self.active.map_or(self.tabs.len(), |a| a + 1);
        let id = self.next_id;
        self.next_id += 1;
        self.tabs.insert(at, Tab { id, path, doc, scroll: 0.0, edit, untitled });
        self.active = Some(at);
        id
    }

    /// Closes a tab. The right neighbour becomes active, or the left one when the
    /// tab was last; none when the set is empty.
    pub fn close(&mut self, id: u64) {
        let Some(i) = self.index_of(id) else { return };
        let tab = self.tabs.remove(i);
        self.remember_closed(tab.path);
        self.active = match self.active {
            Some(a) if a == i => if i < self.tabs.len() { Some(i) } else { i.checked_sub(1) },
            Some(a) if a > i => Some(a - 1),
            other => other,
        };
    }

    /// Closes every tab except `id`, which becomes active.
    #[allow(dead_code)] // The tab context menu (phase 1b) calls this.
    pub fn close_others(&mut self, id: u64) {
        let Some(keep) = self.index_of(id) else { return };
        self.retain_indices(|i| i == keep, id);
    }

    /// Closes the tabs to the right of `id`. The active tab stays unless it was closed,
    /// in which case `id` becomes active.
    #[allow(dead_code)] // The tab context menu (phase 1b) calls this.
    pub fn close_to_right(&mut self, id: u64) {
        let Some(at) = self.index_of(id) else { return };
        self.retain_indices(|i| i <= at, id);
    }

    /// Reopens the most recently closed path and returns its tab id.
    pub fn reopen_closed(&mut self) -> Option<u64> {
        let path = self.closed.pop()?;
        Some(self.open(&path).0)
    }

    pub fn activate(&mut self, id: u64) {
        if let Some(i) = self.index_of(id) {
            self.active = Some(i);
        }
    }

    /// Activates the tab at a 0-based index: ⌘1 is 0, ⌘8 is 7. Index 8 (⌘9) means
    /// the last tab, whatever the count. Other out-of-range indices do nothing.
    pub fn activate_index(&mut self, i: usize) {
        let target = if i == 8 { self.tabs.len().checked_sub(1) } else { Some(i) };
        if let Some(t) = target.filter(|&t| t < self.tabs.len()) {
            self.active = Some(t);
        }
    }

    /// Moves the active tab forward or back, wrapping at both ends.
    pub fn cycle(&mut self, forward: bool) {
        let n = self.tabs.len();
        if n == 0 {
            return;
        }
        self.active = Some(match (self.active, forward) {
            (None, true) => 0,
            (None, false) => n - 1,
            (Some(a), true) => (a + 1) % n,
            (Some(a), false) => (a + n - 1) % n,
        });
    }

    /// Moves a tab to position `to` (clamped to the end). The active tab stays active.
    pub fn move_tab(&mut self, id: u64, to: usize) {
        let Some(from) = self.index_of(id) else { return };
        let active_id = self.active().map(|t| t.id);
        let tab = self.tabs.remove(from);
        let at = to.min(self.tabs.len());
        self.tabs.insert(at, tab);
        self.active = active_id.and_then(|a| self.index_of(a));
    }

    pub fn active(&self) -> Option<&Tab> {
        self.active.and_then(|i| self.tabs.get(i))
    }

    pub fn active_mut(&mut self) -> Option<&mut Tab> {
        self.active.and_then(|i| self.tabs.get_mut(i))
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Tab> {
        self.tabs.iter_mut().find(|t| t.id == id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Tab> {
        self.tabs.iter()
    }

    /// Tab titles in tab order, disambiguated by `display_titles`.
    pub fn titles(&self) -> Vec<String> {
        let paths: Vec<PathBuf> = self.tabs.iter().map(|t| t.path.clone()).collect();
        display_titles(&paths)
    }

    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    fn index_of(&self, id: u64) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == id)
    }

    fn remember_closed(&mut self, path: PathBuf) {
        self.closed.push(path);
        if self.closed.len() > CLOSED_CAP {
            self.closed.remove(0);
        }
    }

    /// Keeps the tabs whose old index passes `keep`, records the rest as closed, and
    /// keeps the active tab if it survived, otherwise activates `fallback`.
    #[allow(dead_code)] // Used by close_others and close_to_right, which phase 1b's context menu calls.
    fn retain_indices(&mut self, keep: impl Fn(usize) -> bool, fallback: u64) {
        let active_id = self.active().map(|t| t.id);
        let old = std::mem::take(&mut self.tabs);
        for (i, tab) in old.into_iter().enumerate() {
            if keep(i) {
                self.tabs.push(tab);
            } else {
                self.remember_closed(tab.path);
            }
        }
        let target = match active_id {
            Some(a) if self.index_of(a).is_some() => a,
            _ => fallback,
        };
        self.active = self.index_of(target);
    }
}

#[cfg(test)]
#[path = "tabs_tests.rs"]
mod tests;
