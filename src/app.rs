//! App state: open tabs, sidebar roots, view settings, and the work queued for the worker.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rune::{Live, PointerPress, Tx};

use crate::edit::{self, EditBuffer};
use crate::find::Find;
use crate::images::ImageCache;
use crate::load::{DocState, LoadError, LARGE_BYTES};
use crate::paths::canonical;
use crate::progress::Progress;
use crate::sidebar::{build_tree, readme_or_first, Sidebar};
use crate::tabs::TabSet;
use crate::watch::{content_hash, FileEvent};

/// Most recently opened files kept for the Open menu.
const RECENT_CAP: usize = 16;
/// How long a toast stays on screen, in seconds.
const TOAST_SECS: f64 = 2.4;
/// Zoom step and limits. Zoom multiplies the reading sizes (§4.7).
const ZOOM_STEP: f32 = 0.1;
const ZOOM_MIN: f32 = 0.6;
const ZOOM_MAX: f32 = 2.0;

/// How a document is shown: the rendered preview, or the editor beside a live preview.
pub enum Mode {
    Preview,
    /// Shown by the Edit view (Task 6).
    Edit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuKind {
    Open,
    More,
    /// The right-click menu of tab `id`, open at `AppState::tab_menu_at`.
    TabContext { id: u64 },
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
    /// Writes `text` to `path` atomically. The result comes back as `JobResult::Saved`.
    Save { tab: u64, path: PathBuf, text: String },
    /// Decode one local image (see `images::decode`).
    Image { key: PathBuf },
    /// Decode the background picture at `path`.
    Background { path: PathBuf },
}

pub struct AppState {
    pub tabs: TabSet,
    pub sidebar: Sidebar,
    pub recent: Vec<PathBuf>,
    pub zoom: f32,
    pub wide: bool,
    /// Theme, fonts, background and the rest of the reader's choices (Settings panel).
    pub settings: crate::settings::Settings,
    /// Themes read from the config folder's `themes/`, loaded after launch.
    pub user_themes: Vec<crate::themes::ThemeDef>,
    /// Installed font families for the font pickers, loaded after launch.
    #[allow(dead_code, reason = "filled by the font loader, read by the Settings panel")]
    pub fonts: crate::fonts::FontList,
    pub drop_hover: bool,
    pub menu: Option<MenuKind>,
    /// Where the tab context menu's secondary press landed, in window points.
    pub tab_menu_at: Option<PointerPress>,
    pub toast: Option<Toast>,
    pub jobs: Vec<Job>,
    /// The tab the strip last scrolled to, so a change of active tab reveals it once.
    pub tab_revealed: Option<u64>,
    pub find: Find,
    /// Decoded images, bounded (§5.3).
    pub images: ImageCache,
    /// Set by `poll` each frame; handlers have no clock of their own.
    pub now: f64,
    /// Preview or Edit, set by the Preview | Edit control and ⌘E.
    pub mode: Mode,
    /// The tab whose unsaved-changes guard is up.
    pub guard: Option<u64>,
    /// Tabs to close once their save succeeds (the guard's Save).
    pub close_after_save: Vec<u64>,
    /// The editor's scroll offset and maximum (y), as its scroll viewport reports them.
    pub editor_at: Live<[f32; 2]>,
    /// The preview's scroll offset and maximum (y), reported the same way.
    pub preview_at: Live<[f32; 2]>,
    /// The editor fraction last copied to the preview. Cleared outside Edit, so re-entering it
    /// syncs again.
    pub scroll_synced: Option<f32>,
    /// Whether the titlebar shows this process's memory and CPU (More menu).
    pub stats_on: bool,
    /// The numbers the readout shows, sampled by `stats::tick`.
    pub stats: crate::stats::Stats,
    /// The readout's text, bound live so a new number repaints without rebuilding the view.
    pub stats_text: rune::LiveText,
    /// The last reading, which the next CPU percentage is taken from.
    pub stats_last: Option<crate::stats::Sample>,
    /// Reading progress for the active document: its line and pill, and which viewport was restored.
    pub doc_scroll: Progress,
    // Task B: the background picture.
    pub bg: crate::background::BackgroundState,
    // Task C: the Settings panel.
    /// Whether the Settings panel is up.
    pub settings_open: bool,
    // Task E: the command palette.
    pub palette: crate::commands::Palette,
}

/// The file name shown in toasts: the last path component, or the whole path when there is none.
fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

impl AppState {
    pub fn new() -> Self {
        Self {
            tabs: TabSet::new(),
            sidebar: Sidebar::new(),
            recent: Vec::new(),
            zoom: 1.0,
            wide: false,
            settings: crate::settings::initial(),
            user_themes: Vec::new(),
            fonts: crate::fonts::FontList::default(),
            drop_hover: false,
            menu: None,
            tab_menu_at: None,
            toast: None,
            jobs: Vec::new(),
            tab_revealed: None,
            find: Find::new(),
            images: ImageCache::new(),
            now: 0.0,
            mode: Mode::Preview,
            guard: None,
            close_after_save: Vec::new(),
            editor_at: Live::new([0.0; 2]),
            preview_at: Live::new([0.0; 2]),
            scroll_synced: None,
            stats_on: true,
            stats: crate::stats::Stats::default(),
            stats_text: rune::LiveText::new(""),
            stats_last: None,
            doc_scroll: Progress::new(),
            bg: crate::background::BackgroundState::default(),
            settings_open: false,
            palette: crate::commands::Palette::new(),
        }
    }

    /// Closes a tab, or asks first when it has unsaved edits: the guard then decides.
    pub fn request_close(&mut self, id: u64) {
        let dirty = self.tabs.iter().any(|t| t.id == id && t.edit.as_ref().is_some_and(EditBuffer::dirty));
        if dirty {
            self.guard = Some(id);
        } else {
            self.close_now(id);
        }
    }

    /// Closes the tabs whose position `pick(index, anchor_index)` selects, relative to the tab
    /// `anchor`, skipping any with unsaved edits so a bulk close never loses work.
    pub fn close_clean(&mut self, pick: impl Fn(usize, usize) -> bool, anchor: u64) {
        let ids: Vec<(u64, bool)> = self.tabs.iter().map(|t| (t.id, t.edit.as_ref().is_some_and(EditBuffer::dirty))).collect();
        let Some(at) = ids.iter().position(|(id, _)| *id == anchor) else { return };
        for (i, (id, dirty)) in ids.iter().enumerate() {
            if pick(i, at) && !dirty {
                self.tabs.close(*id);
            }
        }
        self.tabs.activate(anchor);
        self.sync_find();
    }

    /// Closes a tab whatever its edits, and drops its guard if one is up.
    pub fn close_now(&mut self, id: u64) {
        if self.guard == Some(id) {
            self.guard = None;
        }
        self.tabs.close(id);
        self.sync_find();
    }

    /// Opens an untitled tab named after the first free `Untitled.md`. The caller switches
    /// it to Edit.
    pub fn new_untitled(&mut self) {
        let names: Vec<String> = self.tabs.iter().map(|t| file_name(&t.path)).collect();
        let name = edit::untitled_name(&names);
        self.tabs.open_untitled(&name);
        self.sync_find();
    }

    /// Queues a write of tab `id`'s text to `path`. With `then_close`, the tab closes once the
    /// write lands. A tab that is not loaded has nothing to write.
    pub fn queue_save(&mut self, id: u64, path: PathBuf, then_close: bool) {
        let Some(tab) = self.tabs.iter().find(|t| t.id == id) else { return };
        let text = match (&tab.edit, &tab.doc) {
            (Some(buf), _) => buf.text(),
            (None, DocState::Ready { src, .. }) => src.to_string(),
            _ => return,
        };
        self.jobs.push(Job::Save { tab: id, path, text });
        if then_close {
            self.close_after_save.push(id);
        }
    }

    /// A worker finished a save. On success the tab shows the saved text and records its hash,
    /// so the watcher's reload of the same bytes stays silent. The tab then closes if the guard
    /// asked for that. A failed save keeps the tab and its edits.
    pub fn saved(&mut self, tab: u64, text: String, result: Result<(), String>) {
        let now = self.now;
        let close = self.close_after_save.contains(&tab);
        self.close_after_save.retain(|&id| id != tab);
        let Some(t) = self.tabs.get_mut(tab) else { return };
        let name = file_name(&t.path);
        let toast = match result {
            Ok(()) => {
                t.hash = Some(content_hash(&text));
                t.deleted = false;
                t.conflict = false;
                if let Some(buf) = t.edit.as_mut() {
                    buf.mark_saved(&text);
                }
                let large = text.len() as u64 > LARGE_BYTES;
                t.doc = DocState::Ready { src: Arc::from(text.as_str()), large, lossy: false };
                format!("Saved {name}")
            }
            Err(e) => format!("Couldn't save {name}: {e}"),
        };
        self.show_toast(toast, now);
        if close && self.tabs.iter().any(|t| t.id == tab) {
            self.close_now(tab);
        }
        self.sync_find();
    }

    /// A worker finished loading a tab's document. A reload whose text is unchanged does nothing
    /// visible. A changed document replaces the old one, with a toast when its tab is active and
    /// a dot when it isn't. A file that went missing keeps its last content, marked deleted.
    pub fn loaded(&mut self, tab: u64, doc: DocState) {
        self.apply_loaded(tab, doc);
        self.sync_find();
    }

    fn apply_loaded(&mut self, tab: u64, doc: DocState) {
        let now = self.now;
        let active = self.tabs.active().map(|t| t.id) == Some(tab);
        let Some(t) = self.tabs.get_mut(tab) else { return };
        let was_ready = matches!(t.doc, DocState::Ready { .. });
        let was_deleted = t.deleted;
        let hash = match &doc {
            DocState::Ready { src, .. } => Some(content_hash(src)),
            _ => None,
        };
        let Some(hash) = hash else {
            if was_ready && matches!(doc, DocState::Error(LoadError::NotFound)) {
                self.mark_deleted(tab);
                return;
            }
            t.doc = doc;
            t.hash = None;
            t.deleted = false;
            return;
        };
        let same = was_ready && t.hash == Some(hash);
        if same && !was_deleted {
            return;
        }
        // Unsaved edits are never replaced silently: the banner asks, and the text stays as it is.
        if was_ready && !same && t.edit.as_ref().is_some_and(EditBuffer::dirty) {
            t.conflict = true;
            return;
        }
        // A clean buffer takes the new text, so the editor shows what the preview shows.
        if let (Some(buf), DocState::Ready { src, .. }) = (t.edit.as_mut(), &doc)
            && !buf.dirty()
        {
            *buf = EditBuffer::from_src(src);
        }
        t.doc = doc;
        t.hash = Some(hash);
        t.deleted = false;
        if was_ready && !same {
            let name = file_name(&t.path);
            if active {
                self.show_toast(format!("Reloaded {name}"), now);
            } else {
                t.changed = true;
            }
        }
    }

    /// A file or a sidebar root changed on disk (from the watcher). An open file reloads, a
    /// missing one is marked deleted, and a root's folder is scanned again.
    pub fn file_event(&mut self, ev: FileEvent) {
        match ev {
            FileEvent::Changed(path) => {
                if self.is_root(&path) {
                    self.jobs.push(Job::Scan { root: path });
                    return;
                }
                for id in self.tab_ids_for(&path) {
                    self.jobs.push(Job::Load { tab: id, path: path.clone() });
                }
            }
            FileEvent::Removed(path) => {
                if self.is_root(&path) {
                    self.jobs.push(Job::Scan { root: path });
                    return;
                }
                for id in self.tab_ids_for(&path) {
                    self.mark_deleted(id);
                }
            }
        }
    }

    /// Queues a decode for each local image not yet cached or pending. Paths are already resolved
    /// on the worker.
    pub fn want_images(&mut self, paths: Vec<PathBuf>) {
        for p in paths {
            if !self.images.has(&p) && !self.images.pending(&p) {
                self.images.mark_pending(p.clone());
                self.jobs.push(Job::Image { key: p });
            }
        }
    }

    /// A worker finished decoding an image. A failed decode stays pending, so it is not retried.
    pub fn image_ready(&mut self, key: PathBuf, image: Option<(rune::Image, usize)>) {
        if let Some((img, bytes)) = image {
            self.images.insert(key, img, bytes);
        }
    }

    /// Conflict banner, Keep Mine: the edits stay, and the next save writes them over the file.
    pub fn keep_mine_active(&mut self) {
        if let Some(tab) = self.tabs.active_mut() {
            tab.conflict = false;
        }
    }

    /// Conflict banner, Reload: the active tab's edits are dropped and its file is loaded again.
    pub fn reload_active_discarding(&mut self) {
        if let Some(tab) = self.tabs.active_mut() {
            tab.edit = None;
            tab.conflict = false;
        }
        self.requeue_active_load();
    }

    /// Editor to preview scroll sync, one way: when the editor's scroll moves, the preview goes to
    /// the same fraction of its own scroll range. Called once per frame from `poll`. It only acts
    /// when the editor's fraction changed, so the preview scrolls freely otherwise.
    pub fn sync_preview_scroll(&mut self, tx: &mut Tx) {
        if !matches!(self.mode, Mode::Edit) {
            self.scroll_synced = None;
            return;
        }
        let [offset, max] = self.editor_at.get();
        let frac = if max > 0.0 { (offset / max).clamp(0.0, 1.0) } else { 0.0 };
        if self.scroll_synced == Some(frac) {
            return;
        }
        self.scroll_synced = Some(frac);
        let [_, preview_max] = self.preview_at.get();
        tx.scroll_to_instant("preview", 0.0, frac * preview_max);
    }

    /// Marks a tab's file as deleted and toasts it once. The last content stays on screen.
    fn mark_deleted(&mut self, tab: u64) {
        let now = self.now;
        let Some(t) = self.tabs.get_mut(tab) else { return };
        if t.deleted {
            return;
        }
        t.deleted = true;
        let name = file_name(&t.path);
        self.show_toast(format!("{name} was deleted"), now);
    }

    fn is_root(&self, path: &Path) -> bool {
        self.sidebar.roots.iter().any(|r| r.path == path)
    }

    fn tab_ids_for(&self, path: &Path) -> Vec<u64> {
        self.tabs.iter().filter(|t| t.path == path).map(|t| t.id).collect()
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
                if self.tabs.is_empty()
                    && let Some(rel) = readme_or_first(&files)
                {
                    self.open_file(&key.join(rel));
                }
            }
            Err(e) => {
                let r = &mut self.sidebar.roots[i];
                r.scanning = false;
                r.error = Some(e);
            }
        }
    }

    /// Queues a fresh load for the active tab and shows it as loading again. Try Again and
    /// Reload from Disk both use it.
    pub fn requeue_active_load(&mut self) {
        let Some(tab) = self.tabs.active_mut() else { return };
        tab.doc = DocState::Loading;
        let job = Job::Load { tab: tab.id, path: tab.path.clone() };
        if !self.jobs.contains(&job) {
            self.jobs.push(job);
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

