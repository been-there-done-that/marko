//! Live reload: watches the folders of open files and of sidebar roots, debounces the events
//! per path, and hashes content so a save that changed nothing stays silent.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use notify::event::ModifyKind;
use notify::{Event, EventKind, RecursiveMode, Watcher as NotifyWatcher};

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
#[derive(Default, Clone, PartialEq, Eq)]
struct Interest {
    files: Vec<PathBuf>,
    roots: Vec<PathBuf>,
}

/// Watches the folders of open files and roots. `sync` is cheap and never touches the disk:
/// the notify watcher lives on its own thread, which does the watching and unwatching.
pub struct Watcher {
    /// Shared with the event thread, which reads it to filter events.
    interest: Arc<Mutex<Interest>>,
    /// Hands each changed request to the thread that owns the notify watcher.
    requests: Sender<Interest>,
    /// How many requests that thread has finished applying. Read by the tests only.
    #[cfg_attr(not(test), allow(dead_code))]
    applied: Arc<AtomicUsize>,
}

impl Watcher {
    /// Starts the watcher and its threads. `on_event` runs on the event thread.
    pub fn start(on_event: impl Fn(FileEvent) + Send + 'static) -> Result<Watcher, String> {
        let (raw_tx, raw_rx) = mpsc::channel::<Event>();
        let inner = notify::recommended_watcher(move |res: notify::Result<Event>| {
            if let Ok(ev) = res {
                let _ = raw_tx.send(ev);
            }
        })
        .map_err(|e| e.to_string())?;
        Self::from_inner(Box::new(inner), raw_rx, on_event)
    }

    /// The same watcher on notify's polling backend. Used by the tests when the process's
    /// inotify watch budget is already spent by other programs.
    #[cfg(test)]
    pub fn start_polling(on_event: impl Fn(FileEvent) + Send + 'static) -> Result<Watcher, String> {
        let (raw_tx, raw_rx) = mpsc::channel::<Event>();
        let config = notify::Config::default().with_poll_interval(Duration::from_millis(100));
        let inner = notify::PollWatcher::new(
            move |res: notify::Result<Event>| {
                if let Ok(ev) = res {
                    let _ = raw_tx.send(ev);
                }
            },
            config,
        )
        .map_err(|e| e.to_string())?;
        Self::from_inner(Box::new(inner), raw_rx, on_event)
    }

    fn from_inner(
        inner: Box<dyn NotifyWatcher + Send>,
        raw_rx: Receiver<Event>,
        on_event: impl Fn(FileEvent) + Send + 'static,
    ) -> Result<Watcher, String> {
        let interest = Arc::new(Mutex::new(Interest::default()));
        let shared = Arc::clone(&interest);
        thread::Builder::new()
            .name("watch".into())
            .spawn(move || run(raw_rx, shared, on_event))
            .map_err(|e| e.to_string())?;

        let (requests, wanted) = mpsc::channel::<Interest>();
        let applied = Arc::new(AtomicUsize::new(0));
        let done = Arc::clone(&applied);
        thread::Builder::new()
            .name("watch-folders".into())
            .spawn(move || {
                let mut folders = Folders { inner, watched: HashMap::new() };
                for req in wanted {
                    folders.apply(&req.files, &req.roots);
                    done.fetch_add(1, Ordering::SeqCst);
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Watcher { interest, requests, applied })
    }

    /// Asks for the watched folders to follow the open files and roots. Each open file's parent
    /// folder is watched non-recursively, so an atomic-rename save is seen; each root recursively.
    /// The watches are applied on the watcher's own thread, so this returns at once. Does nothing
    /// when the lists are unchanged.
    pub fn sync(&mut self, files: &[PathBuf], roots: &[PathBuf]) {
        {
            let mut interest = self.interest.lock().unwrap_or_else(|e| e.into_inner());
            if interest.files == files && interest.roots == roots {
                return;
            }
            interest.files = files.to_vec();
            interest.roots = roots.to_vec();
        }
        let req = Interest { files: files.to_vec(), roots: roots.to_vec() };
        // The thread only ends when the watcher is dropped, so the send can't fail while we exist.
        let _ = self.requests.send(req);
    }

    /// How many `sync` requests the watcher thread has applied so far.
    #[cfg(test)]
    pub fn applied(&self) -> usize {
        self.applied.load(Ordering::SeqCst)
    }
}

/// The notify watcher and the folders it watches. Lives on the watcher's own thread, since adding
/// a recursive watch walks the directory tree.
struct Folders {
    inner: Box<dyn NotifyWatcher + Send>,
    /// Folders now watched, with the mode each is watched in.
    watched: HashMap<PathBuf, RecursiveMode>,
}

impl Folders {
    /// Brings the watched folders in line with the lists. Folders no longer needed are unwatched.
    fn apply(&mut self, files: &[PathBuf], roots: &[PathBuf]) {
        let want = wanted_dirs(files, roots);
        let stale: Vec<PathBuf> =
            self.watched.iter().filter(|(d, m)| want.get(*d) != Some(*m)).map(|(d, _)| d.clone()).collect();
        for dir in stale {
            let _ = self.inner.unwatch(&dir);
            self.watched.remove(&dir);
        }
        for (dir, mode) in want {
            if !self.watched.contains_key(&dir) {
                match self.inner.watch(&dir, mode) {
                    Ok(()) => {
                        self.watched.insert(dir, mode);
                    }
                    Err(e) => eprintln!("marko: couldn't watch {}: {e}", dir.display()),
                }
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

/// True for the hidden temp file an atomic save creates beside its target.
fn is_atomic_temp(path: &Path) -> bool {
    path.file_name().is_some_and(|n| {
        let n = n.to_string_lossy();
        n.starts_with('.') && n.ends_with(".tmp")
    })
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
        // A save of an open file writes a hidden temp and renames it over the file. Neither is a
        // folder change, so it must not queue a rescan of the whole root.
        if tree
            && !interest.files.contains(path)
            && !is_atomic_temp(path)
            && let Some(root) = interest.roots.iter().find(|r| path.starts_with(r))
            && !keys.contains(root)
        {
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
