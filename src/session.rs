//! The session: what the window had open, saved in prefs and restored at launch, and the
//! instance lock that makes one process the only one that saves.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use rune::prefs::Prefs;
use serde::{Deserialize, Serialize};

use crate::app::{AppState, Appearance};
use crate::paths::{canonical, is_markdown};
use crate::tabs::Tab;

/// The prefs key the session is stored under.
const KEY: &str = "session";
/// How long the state must sit still before it is written.
const QUIET_SECS: f64 = 1.0;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Session {
    pub tabs: Vec<PathBuf>,
    /// Each tab's scroll offset, parallel to `tabs`. Missing in sessions saved before it existed.
    #[serde(default)]
    pub scroll: Vec<f32>,
    pub active: Option<usize>,
    pub roots: Vec<PathBuf>,
    pub sidebar_visible: bool,
    pub zoom: f32,
    pub wide: bool,
    pub appearance: String,
    pub recent: Vec<PathBuf>,
    pub frame: Option<(i32, i32, u32, u32)>,
}

impl Default for Session {
    /// What a first launch looks like: sidebar shown, 100% zoom, system appearance.
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            scroll: Vec::new(),
            active: None,
            roots: Vec::new(),
            sidebar_visible: true,
            zoom: 1.0,
            wide: false,
            appearance: appearance_name(Appearance::System).into(),
            recent: Vec::new(),
            frame: None,
        }
    }
}

fn appearance_name(a: Appearance) -> &'static str {
    match a {
        Appearance::System => "system",
        Appearance::Light => "light",
        Appearance::Dark => "dark",
    }
}

fn appearance_from(name: &str) -> Appearance {
    match name {
        "light" => Appearance::Light,
        "dark" => Appearance::Dark,
        _ => Appearance::System,
    }
}

/// Reads the window's state into a session. `frame` is left empty for the caller to fill.
pub fn capture(s: &AppState) -> Session {
    // An untitled tab has no file yet, so it is not restored.
    let saved: Vec<&Tab> = s.tabs.iter().filter(|t| !t.untitled).collect();
    let active_id = s.tabs.active().map(|t| t.id);
    Session {
        tabs: saved.iter().map(|t| t.path.clone()).collect(),
        scroll: saved.iter().map(|t| t.scroll).collect(),
        active: active_id.and_then(|id| saved.iter().position(|t| t.id == id)),
        roots: s.sidebar.roots.iter().map(|r| r.path.clone()).collect(),
        sidebar_visible: s.sidebar.visible,
        zoom: s.zoom,
        wide: s.wide,
        appearance: appearance_name(s.appearance).into(),
        recent: s.recent.clone(),
        frame: None,
    }
}

/// Opens a saved session. Paths that no longer exist are skipped and dropped from Recent.
/// Roots open before tabs, and the saved active tab is focused last.
pub fn restore(s: &mut AppState, session: Session, now: f64) {
    let active_path = session
        .active
        .and_then(|i| session.tabs.get(i))
        .filter(|p| p.is_file())
        .map(|p| canonical(p));
    let roots: Vec<PathBuf> = session.roots.iter().filter(|p| p.is_dir()).cloned().collect();
    let mut tabs: Vec<PathBuf> = Vec::new();
    let mut scrolls: Vec<(PathBuf, f32)> = Vec::new();
    for (i, p) in session.tabs.iter().enumerate().filter(|(_, p)| p.is_file() && is_markdown(p)) {
        tabs.push(p.clone());
        scrolls.push((canonical(p), session.scroll.get(i).copied().unwrap_or(0.0).max(0.0)));
    }
    let recent: Vec<PathBuf> = session.recent.iter().filter(|p| p.exists()).cloned().collect();

    s.sidebar.visible = session.sidebar_visible;
    s.zoom = if session.zoom > 0.0 { session.zoom } else { 1.0 };
    s.wide = session.wide;
    s.appearance = appearance_from(&session.appearance);

    s.open_paths(&roots, now);
    s.open_paths(&tabs, now);
    // A tab's offset is applied once its document is shown (`progress`), so it is only recorded here.
    for (path, offset) in scrolls {
        let id = s.tabs.iter().find(|t| t.path == path).map(|t| t.id);
        if let Some(tab) = id.and_then(|id| s.tabs.get_mut(id)) {
            tab.scroll = offset;
        }
    }
    if let Some(key) = active_path {
        let id = s.tabs.iter().find(|t| t.path == key).map(|t| t.id);
        if let Some(id) = id {
            s.tabs.activate(id);
        }
    }
    // Opening the tabs and roots above bumped them in Recent; the saved order is the truth.
    s.recent = recent;
}

/// The saved session, or the default when none is stored or it doesn't parse.
pub fn load(prefs: &Prefs) -> Session {
    prefs.get(KEY).unwrap_or_default()
}

/// Stores the session. A failed write is reported on stderr; the session stays in memory.
pub fn save(prefs: &mut Prefs, session: &Session) {
    if let Err(e) = prefs.set(KEY, session) {
        eprintln!("rune-markdown: {e}");
    }
}

/// Holds an exclusive lock on `instance.lock`. Only the holder saves the session.
pub struct InstanceLock(#[allow(dead_code)] File); // Never read: holding the file keeps the lock.

/// Takes the lock in `dir`, creating the folder if needed. `None` when another process holds it,
/// or when the lock file can't be opened or locked.
pub fn primary_lock(dir: &Path) -> Option<InstanceLock> {
    std::fs::create_dir_all(dir).ok()?;
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(dir.join("instance.lock"))
        .ok()?;
    match file.try_lock() {
        Ok(()) => Some(InstanceLock(file)),
        Err(TryLockError::WouldBlock) | Err(TryLockError::Error(_)) => None,
    }
}

/// Turns a stream of changes into one save: due once, after the state has been quiet for a second.
#[derive(Default)]
pub struct SaveDebounce {
    last_change: Option<f64>,
}

impl SaveDebounce {
    pub fn touch(&mut self, now: f64) {
        self.last_change = Some(now);
    }

    pub fn due(&mut self, now: f64) -> bool {
        match self.last_change {
            Some(t) if now - t >= QUIET_SECS => {
                self.last_change = None;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
#[path = "session_tests.rs"]
mod tests;
