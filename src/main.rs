//! Marko: a fast, light Markdown viewer and editor.

// A release build on Windows opens no console window beside the app.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

mod app;
mod background;
mod cli;
mod commands;
mod detach;
mod edit;
mod find;
mod fonts;
mod gpu_env;
mod icons;
mod images;
mod load;
mod net;
mod open;
mod paths;
mod progress;
mod recovery;
mod session;
mod settings;
mod settings_sync;
mod sidebar;
mod stats;
mod tabs;
mod theme;
mod themes;
mod ui;
mod update;
mod version;
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

/// The app id; its prefs folder holds the session and the user's themes.
const APP_ID: &str = "dev.marko.app";
/// The id the app had before it was named Marko; its prefs move over on the first launch.
const OLD_APP_ID: &str = "dev.rune.markdown";
/// The window class (X11 `WM_CLASS`, Wayland `app_id`) the desktop matches to `marko.desktop`
/// and its icon.
const WINDOW_CLASS: &str = "marko";
/// How long a changed session waits before it is saved. Slightly longer than the debounce's
/// own quiet period, so the wake-up that checks it always finds it due.
const SAVE_WAKE: Duration = Duration::from_millis(1100);
/// How often a dirty buffer is snapshotted for crash recovery.
const RECOVERY_EVERY: f64 = 5.0;

/// The worker's job queue. Set once the window is ready.
static JOBS: OnceLock<Sender<Job>> = OnceLock::new();
/// Saving state, set once the window is ready.
static PERSIST: OnceLock<Mutex<Persist>> = OnceLock::new();
/// The recovery writer, set once the window is ready (primary instance only).
static RECOVERY_TX: OnceLock<Sender<RecoveryMsg>> = OnceLock::new();
/// Live reload, started once the window is ready. Its watched folders follow the tabs and roots.
static WATCHER: OnceLock<Mutex<Watcher>> = OnceLock::new();

thread_local! {
    /// The window, for reading its place when the session is saved. UI thread only.
    static WINDOW: std::cell::RefCell<Option<rune::WindowHandle>> = const { std::cell::RefCell::new(None) };
}

struct Persist {
    /// The window's frame as last read, kept when the window can't report one (maximised, or a
    /// platform that hides its position).
    frame: Option<(i32, i32, u32, u32)>,
    /// Held by the one process that saves. None in the others, and in headless renders,
    /// which never reach `on_ready`.
    primary: Option<InstanceLock>,
    /// The session as last seen by `poll`, as JSON, to tell when anything changed.
    last: String,
    debounce: SaveDebounce,
    /// Sends due snapshots to the saver thread, the only thread that writes the prefs.
    saves: Sender<Session>,
    /// When the last crash-recovery snapshot was taken.
    recovery_at: f64,
}

/// Work for the recovery writer thread.
enum RecoveryMsg {
    Snapshot(Vec<recovery::Recovered>),
    /// Asks the writer to acknowledge everything queued before it, so a close can wait for the
    /// final snapshot to land instead of racing an older one.
    Flush(Sender<()>),
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

/// Three tabs of mixed name lengths, the middle one long enough to be cut, for the tab strip.
/// The long copy goes to the snapshot folder under `target/`, as a scene only.
fn scene_tabs(s: &mut AppState) {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/snaps");
    let long = dir.join("Infrastructure-PR-52-billing-revenue-reconciliation.md");
    if std::fs::create_dir_all(&dir).is_ok() {
        let _ = std::fs::copy(fixtures().join("hello.md"), &long);
    }
    s.open_paths(&[fixtures().join("basic.md"), long, fixtures().join("frontmatter.md")], 0.0);
    settle(s);
}

/// Opens `basic.md` and settles its load, so a scene starts from a rendered document.
fn scene_basic(s: &mut AppState) {
    s.open_paths(&[fixtures().join("basic.md")], 0.0);
    settle(s);
}

/// `images.md` with its pictures decoded: an SVG, a PNG and a web image waiting for Load.
fn scene_images(s: &mut AppState) {
    s.open_paths(&[fixtures().join("images.md")], 0.0);
    settle(s);
}

/// The gradient picture open full size.
fn scene_lightbox(s: &mut AppState) {
    scene_images(s);
    let path = fixtures().join("gradient.png");
    if let Some((image, _)) = images::decode(&path, images::MAX_DECODE_WIDTH) {
        s.lightbox = Some(app::Lightbox { image, alt: "A colour gradient".into() });
    }
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

/// The editor drawing in an installed monospaced family with ligatures on.
fn scene_editor_font(s: &mut AppState) {
    s.settings.code_font = "DejaVu Sans Mono".into();
    s.settings.ligatures = true;
    scene_dirty_editor(s);
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

/// The command palette over a document, filtered by "the".
fn scene_palette(s: &mut AppState) {
    scene_basic(s);
    s.recent = vec![fixtures().join("basic.md")];
    s.palette.show();
    s.palette.query.replace_text("the");
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

/// The fixture picture behind the whole window, with frosted panels.
fn scene_background(s: &mut AppState) {
    scene_folder(s);
    s.settings.background.path = Some(fixtures().join("bg.jpg"));
    let mut tx = Tx::default();
    background::tick(s, 0.0, &mut tx);
    settle(s);
}

/// The Settings panel on tab `tab`, over a document, with a font list filled in by hand.
fn scene_settings(s: &mut AppState, tab: usize) {
    scene_basic(s);
    s.settings_open = true;
    s.settings_tab = tab;
    s.fonts = fonts::FontList {
        all: ["DejaVu Sans", "DejaVu Sans Mono", "Inter", "Noto Serif"].map(String::from).to_vec(),
        mono: ["DejaVu Sans Mono", "Noto Sans Mono"].map(String::from).to_vec(),
        loaded: true,
    };
}

/// A named scene for `--snap`: a label and the setup that puts the app in that state.
type Scene = (&'static str, fn(&mut AppState));

static SCENES: &[Scene] = &[
    ("empty", |_: &mut AppState| {}),
    ("folder", scene_folder),
    ("files", scene_files),
    ("tabs", scene_tabs),
    ("menu-open", |s: &mut AppState| s.menu = Some(MenuKind::Open)),
    ("drop", |s: &mut AppState| s.drop_hover = true),
    ("find", scene_find),
    ("editor", scene_dirty_editor),
    ("editor-font", scene_editor_font),
    ("guard", scene_guard),
    ("deleted", scene_deleted),
    ("conflict", scene_conflict),
    ("code", scene_basic),
    ("background", scene_background),
    ("background-dark", |s: &mut AppState| { scene_background(s); s.settings.theme = "dark".into(); }),
    ("background-sharp", |s: &mut AppState| { scene_background(s); s.settings.background.blur = 0.0; }),
    ("settings-appearance", |s: &mut AppState| scene_settings(s, 0)),
    ("settings-text", |s: &mut AppState| scene_settings(s, 1)),
    ("settings-background", |s: &mut AppState| scene_settings(s, 2)),
    ("settings-general", |s: &mut AppState| scene_settings(s, 3)),
    ("images-svg", scene_images),
    ("lightbox", scene_lightbox),
    ("theme-paper", |s: &mut AppState| { scene_basic(s); s.settings.theme = "paper".into(); }),
    ("palette", scene_palette),
    ("tree-dark", |s: &mut AppState| {
        let root = fixtures().join("..").join("docs");
        s.open_paths(&[root], 0.0);
        settle(s);
        if let Some(r) = s.sidebar.roots.first_mut() {
            r.tree.expand_all();
        }
        s.settings.theme = "dark".into();
    }),
    ("no-sidebar", |s: &mut AppState| { scene_folder(s); s.sidebar.visible = false; }),
    ("folder-dark", |s: &mut AppState| { scene_folder(s); s.settings.theme = "dark".into(); }),
    ("folder-light", |s: &mut AppState| { scene_folder(s); s.settings.theme = "light".into(); }),
    ("background-folder-dark", |s: &mut AppState| { scene_background(s); s.settings.theme = "dark".into(); s.settings.background.blur = 0.0; }),
    ("theme-nordic", |s: &mut AppState| { scene_basic(s); s.settings.theme = "nordic".into(); }),
];

fn main() -> std::process::ExitCode {
    // Version, help and update run in this terminal. They must come before `detach`, which
    // otherwise hands the prompt back with no output.
    if let Some(code) = cli::run() {
        return std::process::ExitCode::from(code as u8);
    }
    // From a terminal, give the terminal back: the window runs in a detached copy.
    detach::maybe_detach();
    // SAFETY: single-threaded, before any thread starts, so nothing else reads or writes the environment.
    #[cfg(target_os = "linux")]
    unsafe {
        std::env::remove_var("WAYLAND_DISPLAY");
        gpu_env::apply();
    }
    rune::prewarm_gpu();
    rune::set_app_id(WINDOW_CLASS);
    // A `--snap` render draws a scene from defaults: the reader's saved settings and window
    // place would make it depend on the machine.
    let headless = std::env::args().any(|a| a == "--snap" || a.starts_with("--snap="));
    let mut frame = None;
    if !headless {
        session::migrate_prefs(&Prefs::open(OLD_APP_ID), &Prefs::open(APP_ID));
        // Read before the window opens, so the first frame already has the reader's theme and
        // fonts. Settings shared by every window come from settings.json; a profile from before
        // that file existed takes them from its saved session.
        let prefs = Prefs::open(APP_ID);
        let saved = session::load(&prefs);
        let shared = prefs.path().and_then(|p| p.parent()).and_then(settings_sync::read);
        if let Some(shared) = &shared {
            // Already on disk: nothing to write until it changes.
            settings_sync::start_with(shared);
        }
        settings::set_initial(shared.unwrap_or_else(|| session::settings_of(&saved)));
        // Reopen where the window was left, when the saved frame is sane.
        frame = saved.frame.and_then(session::usable_frame);
    }
    let spec = AppSpec {
        min_size: Some((560.0, 400.0)),
        on_key: Some(ui::keys::on_key),
        poll: Some(poll),
        scenes: SCENES,
        ..AppSpec::new("Marko", (1100.0, 760.0), AppState::new, ui::view)
    };
    let mut options = rune::WindowOptions::default();
    if let Some((position, size)) = frame {
        options.position = Some(position);
        options.physical_size = Some(size);
    }
    let app = rune::App::new(spec)
        .window_options(options)
        .on_window(|win| WINDOW.with(|w| *w.borrow_mut() = Some(win.clone())))
        .on_close(recovery_on_close)
        .on_quit(save_on_quit)
        .titlebar(rune::Titlebar::new(theme::BAR_H));
    #[cfg(target_os = "macos")]
    let app = app.standard_menus();
    let mut ready_handle = Some(app.handle());
    app.on_ready(move |_| {
        if let Some(handle) = ready_handle.take() {
            start(handle);
        }
    })
    .run();
    std::process::ExitCode::SUCCESS
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
    if let Some(config) = prefs.path().and_then(|p| p.parent()) {
        settings_sync::spawn(config.to_path_buf(), handle.clone());
    }
    if let Some(dir) = prefs.path().and_then(|p| p.parent()).map(|d| d.join("themes")) {
        let themes_handle = handle.clone();
        let _ = thread::Builder::new().name("themes".into()).spawn(move || {
            let (themes, _errors) = themes::load_dir(&dir);
            if !themes.is_empty() {
                themes_handle.update(move |s, _| s.user_themes = themes);
            }
        });
    }
    let fonts_handle = handle.clone();
    let _ = thread::Builder::new().name("fonts".into()).spawn(move || {
        let list = fonts::load();
        fonts_handle.update(move |s, _| s.fonts = list);
    });
    let saved = session::load(&prefs);
    let config_dir = prefs.path().and_then(|p| p.parent().map(|p| p.to_path_buf()));
    let primary = config_dir.as_deref().and_then(session::primary_lock);
    let is_primary = primary.is_some();
    // Only the primary window restores recovery, so two windows don't reopen the same buffers.
    let recovered = if is_primary {
        config_dir.as_deref().map(recovery::read).unwrap_or_default()
    } else {
        Vec::new()
    };
    let (saves, queued) = mpsc::channel::<Session>();
    thread::Builder::new()
        .name("session-save".into())
        .spawn(move || {
            for snapshot in queued {
                session::save(&mut prefs, &snapshot);
            }
        })
        .expect("the OS refused to start the session saver thread");
    let _ = PERSIST.set(Mutex::new(Persist { frame: saved.frame, primary, last: String::new(), debounce: SaveDebounce::default(), saves, recovery_at: 0.0 }));
    if is_primary && let Some(dir) = config_dir {
        let (rec_tx, rec_rx) = mpsc::channel::<RecoveryMsg>();
        let _ = thread::Builder::new().name("recovery-save".into()).spawn(move || {
            while let Ok(msg) = rec_rx.recv() {
                match msg {
                    RecoveryMsg::Snapshot(mut items) => {
                        // Keep only the last of a burst, but stop at a flush so its ack is not lost.
                        loop {
                            match rec_rx.try_recv() {
                                Ok(RecoveryMsg::Snapshot(next)) => items = next,
                                Ok(RecoveryMsg::Flush(ack)) => {
                                    if let Err(e) = recovery::write(&dir, &items) {
                                        eprintln!("marko: couldn't write the recovery file: {e}");
                                    }
                                    let _ = ack.send(());
                                    break;
                                }
                                Err(_) => {
                                    if let Err(e) = recovery::write(&dir, &items) {
                                        eprintln!("marko: couldn't write the recovery file: {e}");
                                    }
                                    break;
                                }
                            }
                        }
                    }
                    // Everything queued before the flush has been written already.
                    RecoveryMsg::Flush(ack) => {
                        let _ = ack.send(());
                    }
                }
            }
        });
        let _ = RECOVERY_TX.set(rec_tx);
    }

    let cli: Vec<PathBuf> = rune::extra_args().iter().filter(|a| !detach::is_flag(a)).map(PathBuf::from).collect();
    handle.update(move |s, _| {
        let now = s.now;
        session::restore(s, saved, now);
        s.open_paths(&cli, now);
        recovery::restore(s, recovered);
    });
}

/// Once per frame: hands queued jobs to the worker, clears an expired toast, and saves the
/// session once it has been quiet for a while. Returns true only when the screen changed.
fn poll(s: &mut AppState, now: f64, tx: &mut Tx) -> bool {
    s.now = now;
    s.sync_preview_scroll(tx);
    let preview_changed = s.catch_up_preview(now, tx);
    s.sync_edit_images();
    s.sync_visible_images();
    ui::tab_strip::reveal_active(s, tx);
    let stats_changed = stats::tick(s, now, tx);
    settings_sync::tick(s, tx);
    let bg_changed = background::tick(s, now, tx);
    let progress_changed = s.sync_doc_scroll();
    if let Some(watcher) = WATCHER.get() {
        let mut files: Vec<PathBuf> = s.tabs.iter().filter(|t| !t.untitled).map(|t| t.path.clone()).collect();
        // Cached pictures are followed too, so an edited image file is decoded again.
        files.extend(s.images.local_paths());
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
            let mut snapshot = session::capture(s);
            let json = serde_json::to_string(&snapshot).unwrap_or_default();
            if json != p.last {
                p.last = json;
                p.debounce.touch(now);
                // Without a frame scheduled, no poll would come to save a change.
                tx.wake_after(SAVE_WAKE);
            }
            if p.debounce.due(now) {
                // The window is asked only now, when a save is about to go out.
                let placed = WINDOW.with(|w| w.borrow().as_ref().and_then(|w| w.placement()));
                if let Some(frame) = placed.as_ref().and_then(session::frame_of) {
                    p.frame = Some(frame);
                }
                snapshot.frame = p.frame;
                let _ = p.saves.send(snapshot);
            }
            if now - p.recovery_at >= RECOVERY_EVERY {
                p.recovery_at = now;
                if let Some(tx) = RECOVERY_TX.get() {
                    let _ = tx.send(RecoveryMsg::Snapshot(recovery::snapshot(s)));
                }
            }
        }
    }
    cleared || stats_changed || progress_changed || bg_changed || preview_changed
}

/// Writes the session one last time as the app quits, with the window's frame, so a change
/// made in the last second (or a window moved without any other change) isn't lost. Runs on the
/// UI thread and writes synchronously; the prefs file is replaced through a rename, so a save
/// the saver thread makes at the same time can't leave it half-written.
fn save_on_quit(s: &mut AppState, _: &mut Tx) {
    if let Some(persist) = PERSIST.get() {
        let mut p = persist.lock().unwrap_or_else(|e| e.into_inner());
        if p.primary.is_some() {
            let placed = WINDOW.with(|w| w.borrow().as_ref().and_then(|w| w.placement()));
            if let Some(frame) = placed.as_ref().and_then(session::frame_of) {
                p.frame = Some(frame);
            }
            let mut snapshot = session::capture(s);
            snapshot.frame = p.frame;
            session::save(&mut Prefs::open(APP_ID), &snapshot);
        }
    }
    // After the session guard is released: recovery takes the same lock.
    write_recovery(s);
}

/// Snapshots unsaved buffers as the window closes, so the next launch can restore them. Rune
/// cannot cancel a close, so this is the only place the edits can be kept.
fn recovery_on_close(s: &mut AppState, _: &mut Tx) {
    write_recovery(s);
}

/// Writes the dirty buffers to the config folder, or clears the snapshot when there are none.
/// Only the primary window writes, matching the session.
fn write_recovery(s: &AppState) {
    let Some(persist) = PERSIST.get() else { return };
    let primary = persist.lock().unwrap_or_else(|e| e.into_inner()).primary.is_some();
    if !primary {
        return;
    }
    let Some(dir) = Prefs::open(APP_ID).path().and_then(|p| p.parent().map(|p| p.to_path_buf())) else { return };
    let items = recovery::snapshot(s);
    // Hand the final snapshot to the writer and wait for it, so an older queued snapshot cannot
    // land after this one. Fall back to a synchronous write when the writer is gone.
    if let Some(tx) = RECOVERY_TX.get() {
        let (ack_tx, ack_rx) = mpsc::channel();
        if tx.send(RecoveryMsg::Snapshot(items.clone())).is_ok()
            && tx.send(RecoveryMsg::Flush(ack_tx)).is_ok()
            && ack_rx.recv_timeout(Duration::from_millis(500)).is_ok()
        {
            return;
        }
    }
    if let Err(e) = recovery::write(&dir, &items) {
        eprintln!("marko: couldn't write the recovery file: {e}");
    }
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
