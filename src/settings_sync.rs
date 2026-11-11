//! Keeps every open Marko window on the same settings and themes.
//!
//! Settings live in `<config>/settings.json`, apart from the session (which only the first window
//! saves). Any window writes the file when its settings change, through a temporary file and a
//! rename, and every window watches the config folder: a change another window wrote is applied
//! here, and a theme file added to or edited in `themes/` reloads the theme list.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use notify::{Event, RecursiveMode, Watcher as _};
use rune::{AppHandle, Tx};

use crate::app::AppState;
use crate::settings::Settings;

/// The settings file's name in the config folder.
pub const FILE: &str = "settings.json";
/// How long a burst of changes (a slider drag) settles before it is written.
const WRITE_QUIET: Duration = Duration::from_millis(250);

/// The settings this window last wrote or took from the file. A change in state that differs
/// from it is this window's own and gets written; a file that matches it is an echo.
static SYNCED: Mutex<Option<Settings>> = Mutex::new(None);
/// What this window's writer last put in the file. The watcher reads every write back; a file
/// that matches it is this window's own echo, even when the window has moved on since (a slider
/// still being dragged), and applying it would rewind the window to that older value.
static WRITTEN: Mutex<Option<Settings>> = Mutex::new(None);
/// Sends settings to the writer thread. Set once the window is ready; headless renders have none.
static WRITES: OnceLock<Sender<Settings>> = OnceLock::new();

fn synced() -> std::sync::MutexGuard<'static, Option<Settings>> {
    SYNCED.lock().unwrap_or_else(|e| e.into_inner())
}

/// The settings saved in `dir`, or None when there is no readable file.
pub fn read(dir: &Path) -> Option<Settings> {
    let text = std::fs::read_to_string(dir.join(FILE)).ok()?;
    serde_json::from_str::<Settings>(&text).ok().map(Settings::clamped)
}

/// Writes `settings` into `dir` atomically: a temporary file in the same folder, then a rename.
pub fn write(dir: &Path, settings: &Settings) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let json = serde_json::to_vec_pretty(settings).map_err(std::io::Error::other)?;
    let tmp = dir.join(format!(".{FILE}.{}.tmp", std::process::id()));
    std::fs::write(&tmp, json)?;
    std::fs::rename(&tmp, dir.join(FILE))
}

/// Records what this window starts with, so the first `tick` doesn't write it straight back.
pub fn start_with(settings: &Settings) {
    *synced() = Some(settings.clone());
}

/// Once per frame: queues this window's own settings changes for the writer. Never redraws.
pub fn tick(s: &AppState, _tx: &mut Tx) {
    let Some(writes) = WRITES.get() else { return };
    if remember_own(&s.settings) {
        let _ = writes.send(s.settings.clone());
    }
}

/// Records `settings` as this window's own latest. Returns true when they changed, so they need
/// writing.
pub fn remember_own(settings: &Settings) -> bool {
    let mut last = synced();
    if last.as_ref() == Some(settings) { return false; }
    *last = Some(settings.clone());
    true
}

/// Records what the writer is about to put in the file, before it does, so the watcher's read
/// of it is known as this window's own.
pub fn note_written(settings: &Settings) {
    *WRITTEN.lock().unwrap_or_else(|e| e.into_inner()) = Some(settings.clone());
}

/// Applies settings another window wrote. An echo of this window's own write changes nothing,
/// even an older one.
pub fn apply_external(s: &mut AppState, theirs: Settings) -> bool {
    if WRITTEN.lock().unwrap_or_else(|e| e.into_inner()).as_ref() == Some(&theirs) {
        return false;
    }
    let mut last = synced();
    if last.as_ref() == Some(&theirs) || s.settings == theirs {
        *last = Some(theirs);
        return false;
    }
    *last = Some(theirs.clone());
    s.settings = theirs;
    true
}

/// Starts the writer thread and the watcher on `dir` (the config folder). Called from `on_ready`.
pub fn spawn(dir: PathBuf, handle: AppHandle<AppState>) {
    let (tx, rx) = mpsc::channel::<Settings>();
    let write_dir = dir.clone();
    let spawned = std::thread::Builder::new().name("settings-save".into()).spawn(move || {
        while let Ok(mut latest) = rx.recv() {
            // Keep only the last of a burst.
            while let Ok(next) = rx.recv_timeout(WRITE_QUIET) {
                latest = next;
            }
            note_written(&latest);
            if let Err(e) = write(&write_dir, &latest) {
                eprintln!("marko: couldn't save settings: {e}");
            }
        }
    });
    if spawned.is_ok() {
        let _ = WRITES.set(tx);
    }

    let themes = dir.join("themes");
    let _ = std::fs::create_dir_all(&themes);
    let watch_dir = dir.clone();
    let _ = std::thread::Builder::new().name("settings-watch".into()).spawn(move || {
        let (events, incoming) = mpsc::channel::<Event>();
        let Ok(mut watcher) = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(ev) = res {
                let _ = events.send(ev);
            }
        }) else { return };
        if watcher.watch(&watch_dir, RecursiveMode::NonRecursive).is_err() {
            return;
        }
        let _ = watcher.watch(&themes, RecursiveMode::NonRecursive);
        while let Ok(first) = incoming.recv() {
            // Let a burst (write, rename, metadata) settle, then look once.
            let mut batch = vec![first];
            while let Ok(ev) = incoming.recv_timeout(Duration::from_millis(120)) {
                batch.push(ev);
            }
            let touched = |want: &dyn Fn(&Path) -> bool| batch.iter().flat_map(|e| &e.paths).any(|p| want(p));
            if touched(&|p| p.file_name().is_some_and(|n| n == FILE))
                && let Some(theirs) = read(&watch_dir)
            {
                handle.update(move |s, _| {
                    apply_external(s, theirs);
                });
            }
            if touched(&|p| p.starts_with(&themes)) {
                let (list, _errors) = crate::themes::load_dir(&themes);
                handle.update(move |s, _| s.user_themes = list);
            }
        }
    });
}

#[cfg(test)]
#[path = "settings_sync_tests.rs"]
mod tests;
