//! Rune Markdown: a light Markdown viewer.

mod app;
mod edit;
mod find;
mod icons;
mod images;
mod load;
mod open;
mod paths;
mod session;
mod sidebar;
mod tabs;
mod theme;
mod ui;
mod watch;
mod worker;

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use rune::prefs::Prefs;
use rune::{AppHandle, AppSpec, Tx};

use app::{AppState, Job, MenuKind};
use session::{InstanceLock, SaveDebounce, Session};
use watch::{FileEvent, Watcher};

/// The app id; its prefs folder holds the session.
const APP_ID: &str = "dev.rune.markdown";
/// How long a changed session waits before it is saved. Slightly longer than the debounce's
/// own quiet period, so the wake-up that checks it always finds it due.
const SAVE_WAKE: Duration = Duration::from_millis(1100);

/// The worker's job queue. Set once the window is ready.
static JOBS: OnceLock<Sender<Job>> = OnceLock::new();
/// Saving state, set once the window is ready.
static PERSIST: OnceLock<Mutex<Persist>> = OnceLock::new();
/// Live reload, started once the window is ready. Its watched folders follow the tabs and roots.
static WATCHER: OnceLock<Mutex<Watcher>> = OnceLock::new();

struct Persist {
    /// Held by the one process that saves. None in the others, and in headless renders,
    /// which never reach `on_ready`.
    primary: Option<InstanceLock>,
    /// The session as last seen by `poll`, as JSON, to tell when anything changed.
    last: String,
    debounce: SaveDebounce,
    /// Sends due snapshots to the saver thread, the only thread that writes the prefs.
    saves: Sender<Session>,
}

/// Runs queued jobs on this thread. Scenes render without a worker, so they settle their loads here.
fn settle(s: &mut AppState) {
    while !s.jobs.is_empty() {
        for job in std::mem::take(&mut s.jobs) {
            worker::run_job(job).apply(s);
        }
    }
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures")
}

fn scene_folder(s: &mut AppState) {
    let dir = fixtures();
    s.open_paths(&[dir.clone(), dir.join("basic.md")], 0.0);
    settle(s);
}

fn scene_files(s: &mut AppState) {
    let dir = fixtures();
    s.open_paths(&[dir.join("basic.md"), dir.join("frontmatter.md")], 0.0);
    settle(s);
}

/// Opens `basic.md` and settles its load, so a scene starts from a rendered document.
fn scene_basic(s: &mut AppState) {
    s.open_paths(&[fixtures().join("basic.md")], 0.0);
    settle(s);
}

/// The active tab's edit buffer, created on first use. Scenes type into it directly.
fn active_buffer(s: &mut AppState) -> Option<&mut edit::EditBuffer> {
    s.tabs.active_mut()?.edit.as_mut()
}

/// Switches the active tab to Edit and adds a few characters, so it is dirty.
fn scene_dirty_editor(s: &mut AppState) {
    scene_basic(s);
    ui::editor::enter(s);
    if let Some(buf) = active_buffer(s) {
        let end = buf.text.text.len();
        buf.text.set_caret(end);
        buf.text.insert("\n\nEdited in place.");
    }
}

fn scene_find(s: &mut AppState) {
    scene_basic(s);
    s.open_find();
    s.find.query.replace_text("table");
    s.sync_find();
}

fn scene_guard(s: &mut AppState) {
    scene_dirty_editor(s);
    if let Some(id) = s.tabs.active().map(|t| t.id) {
        s.request_close(id);
    }
}

fn scene_deleted(s: &mut AppState) {
    scene_basic(s);
    if let Some(path) = s.tabs.active().map(|t| t.path.clone()) {
        s.file_event(FileEvent::Removed(path));
    }
}

fn scene_conflict(s: &mut AppState) {
    scene_dirty_editor(s);
    if let Some(tab) = s.tabs.active_mut() {
        tab.conflict = true;
    }
}

/// A named scene for `--snap`: a label and the setup that puts the app in that state.
type Scene = (&'static str, fn(&mut AppState));

static SCENES: &[Scene] = &[
    ("empty", |_: &mut AppState| {}),
    ("folder", scene_folder),
    ("files", scene_files),
    ("menu-open", |s: &mut AppState| s.menu = Some(MenuKind::Open)),
    ("drop", |s: &mut AppState| s.drop_hover = true),
    ("find", scene_find),
    ("editor", scene_dirty_editor),
    ("guard", scene_guard),
    ("deleted", scene_deleted),
    ("conflict", scene_conflict),
    ("code", scene_basic),
];

fn main() {
    // SAFETY: single-threaded, before any thread starts, so nothing else reads or writes the environment.
    #[cfg(target_os = "linux")]
    unsafe {
        std::env::remove_var("WAYLAND_DISPLAY");
    }
    rune::prewarm_gpu();

    let spec = AppSpec {
        min_size: Some((560.0, 400.0)),
        on_key: Some(ui::keys::on_key),
        poll: Some(poll),
        scenes: SCENES,
        ..AppSpec::new("Markdown", (1100.0, 760.0), AppState::new, ui::view)
    };
    let app = rune::App::new(spec).titlebar(rune::Titlebar::new(theme::BAR_H));
    #[cfg(target_os = "macos")]
    let app = app.standard_menus();
    let mut ready_handle = Some(app.handle());
    app.on_ready(move |_| {
        if let Some(handle) = ready_handle.take() {
            start(handle);
        }
    })
    .run();
}

/// Runs once the window is up: starts the worker, takes the primary lock, restores the saved
/// session, then opens the paths given on the command line.
fn start(handle: AppHandle<AppState>) {
    let _ = JOBS.set(worker::spawn_worker(handle.clone()));
    let watch_handle = handle.clone();
    if let Ok(watcher) = Watcher::start(move |ev| watch_handle.update(move |s, _| s.file_event(ev))) {
        let _ = WATCHER.set(Mutex::new(watcher));
    }

    let mut prefs = Prefs::open(APP_ID);
    let saved = session::load(&prefs);
    let primary = prefs.path().and_then(|p| p.parent()).and_then(session::primary_lock);
    let (saves, queued) = mpsc::channel::<Session>();
    thread::Builder::new()
        .name("session-save".into())
        .spawn(move || {
            for snapshot in queued {
                session::save(&mut prefs, &snapshot);
            }
        })
        .expect("the OS refused to start the session saver thread");
    let _ = PERSIST.set(Mutex::new(Persist { primary, last: String::new(), debounce: SaveDebounce::default(), saves }));

    let cli: Vec<PathBuf> = rune::extra_args().iter().map(PathBuf::from).collect();
    handle.update(move |s, _| {
        let now = s.now;
        session::restore(s, saved, now);
        s.open_paths(&cli, now);
    });
}

/// Once per frame: hands queued jobs to the worker, clears an expired toast, and saves the
/// session once it has been quiet for a while. Returns true only when the screen changed.
fn poll(s: &mut AppState, now: f64, tx: &mut Tx) -> bool {
    s.now = now;
    s.sync_preview_scroll(tx);
    if let Some(watcher) = WATCHER.get() {
        let files: Vec<PathBuf> = s.tabs.iter().filter(|t| !t.untitled).map(|t| t.path.clone()).collect();
        let roots: Vec<PathBuf> = s.sidebar.roots.iter().map(|r| r.path.clone()).collect();
        watcher.lock().unwrap_or_else(|e| e.into_inner()).sync(&files, &roots);
    }
    if let Some(jobs) = JOBS.get() {
        for job in s.jobs.drain(..) {
            let _ = jobs.send(job);
        }
    }
    let cleared = s.tick(now);
    if let Some(toast) = &s.toast {
        tx.wake_after(Duration::from_secs_f64((toast.until - now).max(0.0)));
    }

    if let Some(persist) = PERSIST.get() {
        let mut p = persist.lock().unwrap_or_else(|e| e.into_inner());
        if p.primary.is_some() {
            let snapshot = session::capture(s);
            let json = serde_json::to_string(&snapshot).unwrap_or_default();
            if json != p.last {
                p.last = json;
                p.debounce.touch(now);
                // Without a frame scheduled, no poll would come to save a change.
                tx.wake_after(SAVE_WAKE);
            }
            if p.debounce.due(now) {
                let _ = p.saves.send(snapshot);
            }
        }
    }
    cleared
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scenes_are_unique_names() {
        let mut names: Vec<&str> = SCENES.iter().map(|(n, _)| *n).collect();
        names.sort();
        names.dedup();
        assert_eq!(names.len(), SCENES.len());
    }

    #[test]
    fn folder_scene_opens_root_and_file() {
        let mut s = AppState::new();
        scene_folder(&mut s);
        assert_eq!(s.sidebar.roots.len(), 1);
        assert_eq!(s.tabs.iter().count(), 1);
        assert!(s.jobs.is_empty());
    }
}
