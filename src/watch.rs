//! Live reload: watches the folders of open files and of sidebar roots, debounces the events
//! per path, and hashes content so a save that changed nothing stays silent.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use notify::event::ModifyKind;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};

/// A path is quiet this long before its event is delivered.
const QUIET_SECS: f64 = 0.15;
/// How often the event thread wakes to deliver quiet paths while some are still pending.
const TICK: Duration = Duration::from_millis(50);

/// What happened to a watched path, after its events went quiet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {
    Changed(PathBuf),
    Removed(PathBuf),
}

/// Which paths the event thread passes on: open files, and sidebar roots (a root stands for
/// a create, remove or rename anywhere under it).
#[derive(Default, PartialEq, Eq)]
struct Interest {
    files: Vec<PathBuf>,
    roots: Vec<PathBuf>,
}

pub struct Watcher {
    inner: RecommendedWatcher,
    /// Folders now watched, with the mode each is watched in.
    watched: HashMap<PathBuf, RecursiveMode>,
    /// Shared with the event thread, which reads it to filter events.
    interest: Arc<Mutex<Interest>>,
}

impl Watcher {
    /// Starts the watcher and its debounce thread. `on_event` runs on that thread.
    pub fn start(on_event: impl Fn(FileEvent) + Send + 'static) -> Result<Watcher, String> {
        let (raw_tx, raw_rx) = mpsc::channel::<Event>();
        let inner = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(ev) = res {
                let _ = raw_tx.send(ev);
            }
        })
        .map_err(|e| e.to_string())?;
        let interest = Arc::new(Mutex::new(Interest::default()));
        let shared = Arc::clone(&interest);
        thread::Builder::new()
            .name("watch".into())
            .spawn(move || run(raw_rx, shared, on_event))
            .map_err(|e| e.to_string())?;
        Ok(Watcher { inner, watched: HashMap::new(), interest })
    }

    /// Brings the watched folders in line with the open files and roots. Each open file's parent
    /// folder is watched non-recursively, so an atomic-rename save is seen; each root recursively.
    /// Folders no longer needed are unwatched. Does nothing when the lists are unchanged.
    pub fn sync(&mut self, files: &[PathBuf], roots: &[PathBuf]) {
        {
            let mut interest = self.interest.lock().unwrap_or_else(|e| e.into_inner());
            if interest.files == files && interest.roots == roots {
                return;
            }
            interest.files = files.to_vec();
            interest.roots = roots.to_vec();
        }
        let want = wanted_dirs(files, roots);
        let stale: Vec<PathBuf> =
            self.watched.iter().filter(|(d, m)| want.get(*d) != Some(*m)).map(|(d, _)| d.clone()).collect();
        for dir in stale {
            let _ = self.inner.unwatch(&dir);
            self.watched.remove(&dir);
        }
        for (dir, mode) in want {
            if !self.watched.contains_key(&dir) && self.inner.watch(&dir, mode).is_ok() {
                self.watched.insert(dir, mode);
            }
        }
    }
}

/// The folders that need a watch, and the mode each needs. A file under a root is already
/// covered by the root's recursive watch.
fn wanted_dirs(files: &[PathBuf], roots: &[PathBuf]) -> HashMap<PathBuf, RecursiveMode> {
    let mut want = HashMap::new();
    for root in roots {
        want.insert(root.clone(), RecursiveMode::Recursive);
    }
    for file in files {
        if roots.iter().any(|r| file.starts_with(r)) {
            continue;
        }
        if let Some(parent) = file.parent() {
            want.entry(parent.to_path_buf()).or_insert(RecursiveMode::NonRecursive);
        }
    }
    want
}

/// The event thread: collects raw events into pending paths, and delivers each path once it has
/// been quiet for `QUIET_SECS`. Exits when the watcher is dropped.
fn run(raw: Receiver<Event>, interest: Arc<Mutex<Interest>>, on_event: impl Fn(FileEvent)) {
    let started = Instant::now();
    let mut pending: Vec<(PathBuf, f64)> = Vec::new();
    loop {
        let next = if pending.is_empty() {
            match raw.recv() {
                Ok(ev) => Some(ev),
                Err(_) => return,
            }
        } else {
            match raw.recv_timeout(TICK) {
                Ok(ev) => Some(ev),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        };
        let now = started.elapsed().as_secs_f64();
        if let Some(ev) = next {
            let keys = keys_for(&ev, &interest.lock().unwrap_or_else(|e| e.into_inner()));
            for key in keys {
                match pending.iter_mut().find(|(p, _)| *p == key) {
                    Some(entry) => entry.1 = now,
                    None => pending.push((key, now)),
                }
            }
        }
        for path in debounce(&mut pending, now) {
            on_event(if path.exists() { FileEvent::Changed(path) } else { FileEvent::Removed(path) });
        }
    }
}

/// The paths a raw event means to the app: an open file it touches, and the root it falls under
/// when the event creates, removes or renames something there. Reads and attribute-only changes
/// are ignored, so a reload that reads a file cannot trigger another reload.
fn keys_for(ev: &Event, interest: &Interest) -> Vec<PathBuf> {
    let tree = match ev.kind {
        EventKind::Create(_) | EventKind::Remove(_) | EventKind::Modify(ModifyKind::Name(_)) => true,
        EventKind::Modify(_) => false,
        _ => return Vec::new(),
    };
    let mut keys: Vec<PathBuf> = Vec::new();
    for path in &ev.paths {
        if interest.files.contains(path) {
            keys.push(path.clone());
        }
        if tree && let Some(root) = interest.roots.iter().find(|r| path.starts_with(r)) && !keys.contains(root) {
            keys.push(root.clone());
        }
    }
    keys
}

/// Removes and returns the paths that have been quiet for at least 150 ms, given the time of each
/// path's last event. Paths still moving stay in `events`.
pub fn debounce(events: &mut Vec<(PathBuf, f64)>, now: f64) -> Vec<PathBuf> {
    let mut due = Vec::new();
    events.retain(|(path, last)| {
        if now - *last >= QUIET_SECS {
            due.push(path.clone());
            false
        } else {
            true
        }
    });
    due
}

/// A hash of a document's text, for telling a real change from a save that changed nothing.
pub fn content_hash(src: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    src.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
#[path = "watch_tests.rs"]
mod tests;
