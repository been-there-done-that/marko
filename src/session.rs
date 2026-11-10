//! The session: what the window had open, saved in prefs and restored at launch, and the
//! instance lock that makes one process the only one that saves.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};

use rune::prefs::Prefs;
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::settings::{Settings, Startup};
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
    /// The reader's settings. Missing in sessions saved before it existed; `appearance` then
    /// picks the theme.
    #[serde(default)]
    pub settings: Option<Settings>,
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
            appearance: "system".into(),
            recent: Vec::new(),
            frame: None,
            settings: None,
        }
    }
}

/// The settings a saved session asks for. An old session without them keeps its light or dark
/// choice as the theme.
pub fn settings_of(session: &Session) -> Settings {
    match &session.settings {
        Some(s) => s.clone().clamped(),
        None => {
            let mut s = Settings::default();
            if session.appearance == "light" || session.appearance == "dark" {
                s.theme = session.appearance.clone();
            }
            s
        }
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
        appearance: "system".into(),
        recent: s.recent.clone(),
        frame: None,
        settings: Some(s.settings.clone()),
    }
}

/// The window's size floor in physical pixels when a saved frame is judged: below this the
/// frame is a glitch (a minimised window reports a sliver), not a size the reader chose.
const MIN_FRAME: (u32, u32) = (560, 400);
/// The largest side and farthest corner accepted, past any real desktop.
const MAX_SIDE: u32 = 16_384;
const MAX_OFFSET: i32 = 32_768;

/// The frame to save for a window at `p`, or None while it is maximised, minimised or full
/// screen, which reopen as ordinary windows rather than at that size.
pub fn frame_of(p: &rune::WindowPlacement) -> Option<(i32, i32, u32, u32)> {
    if p.maximized || p.minimized || p.full_screen {
        return None;
    }
    let (w, h) = p.content_size;
    // `position` is the outer frame's corner, which is what `WindowOptions::position` takes.
    let frame = (p.position.0, p.position.1, w, h);
    usable_frame(frame).map(|_| frame)
}

/// A saved frame as the window's position and physical content size, or None when it is not a
/// sane window: smaller than the minimum, larger than any desktop, or absurdly far away.
pub fn usable_frame(frame: (i32, i32, u32, u32)) -> Option<((i32, i32), (u32, u32))> {
    let (x, y, w, h) = frame;
    let sized = (MIN_FRAME.0..=MAX_SIDE).contains(&w) && (MIN_FRAME.1..=MAX_SIDE).contains(&h);
    let placed = x.abs() <= MAX_OFFSET && y.abs() <= MAX_OFFSET;
    (sized && placed).then_some(((x, y), (w, h)))
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
    s.settings = settings_of(&session);
    if s.settings.startup == Startup::Empty {
        s.recent = recent;
        return;
    }

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

/// Moves the prefs folder of an older app id (`from`) to the current one (`to`) on the first
/// launch under the new id: the session file and the `themes/` folder are copied, and the old
/// folder is left as it was. Nothing happens when `to` already has a file, or `from` has none.
pub fn migrate_prefs(from: &Prefs, to: &Prefs) {
    let (Some(old), Some(new)) = (from.path(), to.path()) else { return };
    if new.exists() || !old.is_file() {
        return;
    }
    let (Some(old_dir), Some(new_dir)) = (old.parent(), new.parent()) else { return };
    if std::fs::create_dir_all(new_dir).is_err() || std::fs::copy(old, new).is_err() {
        return;
    }
    let themes = old_dir.join("themes");
    if let Ok(entries) = std::fs::read_dir(&themes) {
        let dest = new_dir.join("themes");
        let _ = std::fs::create_dir_all(&dest);
        for entry in entries.flatten() {
            let _ = std::fs::copy(entry.path(), dest.join(entry.file_name()));
        }
    }
}

/// The saved session, or the default when none is stored or it doesn't parse.
pub fn load(prefs: &Prefs) -> Session {
    prefs.get(KEY).unwrap_or_default()
}

/// Stores the session. A failed write is reported on stderr; the session stays in memory.
pub fn save(prefs: &mut Prefs, session: &Session) {
    if let Err(e) = prefs.set(KEY, session) {
        eprintln!("marko: {e}");
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
