//! App state: open tabs, sidebar roots, view settings, and the work queued for the worker.

use std::path::{Path, PathBuf};

use rune_kit::shell::TabDragState;

use crate::load::DocState;
use crate::paths::canonical;
use crate::sidebar::{build_tree, readme_or_first, Sidebar};
use crate::tabs::TabSet;

/// Most recently opened files kept for the Open menu.
const RECENT_CAP: usize = 16;
/// How long a toast stays on screen, in seconds.
const TOAST_SECS: f64 = 2.4;
/// Zoom step and limits. Zoom multiplies the reading sizes (§4.7).
const ZOOM_STEP: f32 = 0.1;
const ZOOM_MIN: f32 = 0.6;
const ZOOM_MAX: f32 = 2.0;

/// How a document is shown. Edit arrives in phase 2.
pub enum Mode {
    Preview,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Appearance {
    System,
    Light,
    Dark,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    Open,
    More,
}

pub struct Toast {
    pub text: String,
    pub until: f64,
}

/// Work for the worker thread. `main.rs` drains `AppState::jobs`.
#[derive(Debug, PartialEq, Eq)]
pub enum Job {
    Load { tab: u64, path: PathBuf },
    Scan { root: PathBuf },
}

pub struct AppState {
    pub tabs: TabSet,
    pub sidebar: Sidebar,
    pub recent: Vec<PathBuf>,
    pub zoom: f32,
    pub wide: bool,
    pub appearance: Appearance,
    pub drop_hover: bool,
    pub menu: Option<MenuKind>,
    pub toast: Option<Toast>,
    pub jobs: Vec<Job>,
    pub drag: TabDragState,
    /// Set by `poll` each frame; handlers have no clock of their own.
    pub now: f64,
    pub mode: Mode,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tabs: TabSet::new(),
            sidebar: Sidebar::new(),
            recent: Vec::new(),
            zoom: 1.0,
            wide: false,
            appearance: Appearance::System,
            drop_hover: false,
            menu: None,
            toast: None,
            jobs: Vec::new(),
            drag: TabDragState::default(),
            now: 0.0,
            mode: Mode::Preview,
        }
    }

    /// A worker finished loading a tab's document.
    pub fn loaded(&mut self, tab: u64, doc: DocState) {
        if let Some(t) = self.tabs.get_mut(tab) {
            t.doc = doc;
        }
    }

    /// A worker finished scanning a root. Builds its tree; opens the README (or the first
    /// file) when no tab is open yet. An error is shown in the sidebar.
    pub fn scanned(&mut self, root: &Path, files: Result<Vec<PathBuf>, String>) {
        let key = canonical(root);
        let Some(i) = self.sidebar.roots.iter().position(|r| r.path == key) else {
            return;
        };
        match files {
            Ok(files) => {
                let r = &mut self.sidebar.roots[i];
                r.scanning = false;
                r.error = None;
                r.tree = build_tree(&format!("root-{i}"), &files, &key);
                if self.tabs.is_empty() {
                    if let Some(rel) = readme_or_first(&files) {
                        self.open_file(&key.join(rel));
                    }
                }
            }
            Err(e) => {
                let r = &mut self.sidebar.roots[i];
                r.scanning = false;
                r.error = Some(e);
            }
        }
    }

    /// Shows a toast for 2.4 seconds, replacing any toast already up.
    pub fn show_toast(&mut self, text: impl Into<String>, now: f64) {
        self.toast = Some(Toast { text: text.into(), until: now + TOAST_SECS });
    }

    /// Clears the toast once its time is up. Returns true when it cleared.
    pub fn tick(&mut self, now: f64) -> bool {
        match &self.toast {
            Some(t) if now >= t.until => {
                self.toast = None;
                true
            }
            _ => false,
        }
    }

    /// One zoom step in. Zoom stays within 60% to 200%, on a 0.1 grid.
    pub fn zoom_in(&mut self) {
        self.set_zoom(self.zoom + ZOOM_STEP);
    }

    /// One zoom step out, within the same limits as `zoom_in`.
    pub fn zoom_out(&mut self) {
        self.set_zoom(self.zoom - ZOOM_STEP);
    }

    /// Back to 100%.
    pub fn zoom_reset(&mut self) {
        self.zoom = 1.0;
    }

    fn set_zoom(&mut self, zoom: f32) {
        self.zoom = (zoom.clamp(ZOOM_MIN, ZOOM_MAX) * 10.0).round() / 10.0;
    }

    /// Moves a file to the front of Recent, without duplicates, keeping at most 16.
    pub fn remember_recent(&mut self, path: &Path) {
        let key = canonical(path);
        self.recent.retain(|p| *p != key);
        self.recent.insert(0, key);
        self.recent.truncate(RECENT_CAP);
    }
}

