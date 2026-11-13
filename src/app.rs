//! App state: open tabs, sidebar roots, view settings, and the work queued for the worker.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use rune::{Live, PointerPress, Tx};
use rune_kit::{FileNode, TreeModel};

use crate::edit::{self, EditBuffer};
use crate::find::Find;
use crate::images::ImageCache;
use crate::load::{DocState, LoadError, LARGE_BYTES};
use crate::paths::canonical;
use crate::progress::Progress;
use crate::sidebar::Sidebar;
use crate::tabs::TabSet;
use crate::watch::{content_hash, FileEvent};

/// Most recently opened files kept for the Open menu.
const RECENT_CAP: usize = 16;
/// Most web pictures queued at once, so a document naming thousands of images cannot fill the
/// job queue.
const MAX_PENDING_IMAGES: usize = 256;
/// Distinct image decodes in flight at once (queued, decoding, or decoded but not yet applied).
/// This bounds how many decoded pictures can sit in the result channel.
const MAX_IN_FLIGHT_IMAGES: usize = 4;
/// Images decoded on each side of the reader's scroll position. A short document is decoded whole.
const LAZY_IMAGE_WINDOW: usize = 2;
/// How long typing must pause before the editor preview re-renders the document.
const PREVIEW_QUIET: f64 = 0.12;
/// How long a toast stays on screen, in seconds.
const TOAST_SECS: f64 = 2.4;
/// Zoom step and limits. Zoom multiplies the reading sizes (§4.7).
const ZOOM_STEP: f32 = 0.1;
pub(crate) const ZOOM_MIN: f32 = 0.6;
pub(crate) const ZOOM_MAX: f32 = 2.0;

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
    /// Fetch and decode one web image. Queued only when the reader allowed web images
    /// (`AppState::want_web`).
    WebImage { url: String },
}

/// The picture shown full size over the window.
pub struct Lightbox {
    pub image: rune::Image,
    pub alt: String,
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
    /// Appearance, Text, Background or General (0 to 3).
    pub settings_tab: usize,
    pub body_combo: rune_kit::ComboboxState,
    pub code_combo: rune_kit::ComboboxState,
    // Task D (images): the full-size view, web image policy and the editor preview's image sync.
    pub lightbox: Option<Lightbox>,
    /// Tabs whose web images the reader allowed with Load (while the setting is Ask).
    pub web_allowed: HashSet<u64>,
    /// The web image addresses each tab's document or edit buffer names.
    pub web_urls: HashMap<u64, Vec<String>>,
    /// The tab and text hash the editor preview's images were last queued for.
    pub edit_images_seen: Option<(u64, u64)>,
}

/// The file name shown in toasts: the last path component, or the whole path when there is none.
fn file_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| path.display().to_string())
}

impl AppState {
    /// Shows or hides the sidebar in one frame. An animated commit would spring the whole
    /// document column sideways while its text re-wraps, and fade the controls between the
    /// sidebar's header and the toolbar.
    pub fn toggle_sidebar(&mut self, tx: &mut Tx) {
        self.sidebar.visible = !self.sidebar.visible;
        tx.instant();
    }

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
            // Off by default: while on, the readout wakes the app every second even when idle.
            stats_on: false,
            stats: crate::stats::Stats::default(),
            stats_text: rune::LiveText::new(""),
            stats_last: None,
            doc_scroll: Progress::new(),
            bg: crate::background::BackgroundState::default(),
            settings_open: false,
            palette: crate::commands::Palette::new(),
            settings_tab: 0,
            body_combo: rune_kit::ComboboxState::default(),
            code_combo: rune_kit::ComboboxState::default(),
            lightbox: None,
            web_allowed: HashSet::new(),
            web_urls: HashMap::new(),
            edit_images_seen: None,
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
                self.forget_tab(*id);
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
        self.forget_tab(id);
        self.sync_find();
    }

    /// Drops the per-tab web image state of a closed tab, so the maps don't grow with every tab
    /// ever opened.
    fn forget_tab(&mut self, id: u64) {
        self.web_allowed.remove(&id);
        self.web_urls.remove(&id);
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
    /// write lands. `save_as` allows a lossily-loaded document to be written to a new path.
    pub fn queue_save(&mut self, id: u64, path: PathBuf, then_close: bool, save_as: bool) {
        let now = self.now;
        let lossy = self.tabs.iter().find(|t| t.id == id).is_some_and(|t| matches!(t.doc, DocState::Ready { lossy: true, .. }));
        if lossy && !save_as {
            // Loading replaced unreadable bytes with U+FFFD. Writing that back over the same file
            // would corrupt it, so refuse; Save As writes the readable text elsewhere.
            self.show_toast("This file has characters that couldn't be read. Use Save As to keep the original", now);
            return;
        }
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
        let succeeded = result.is_ok();
        let toast = match result {
            Ok(()) => {
                t.hash = Some(content_hash(&text));
                t.deleted = false;
                t.conflict = false;
                if let Some(buf) = t.edit.as_mut() {
                    buf.mark_saved(&text);
                }
                let large = text.len() as u64 > LARGE_BYTES;
                t.doc = DocState::Ready { src: Arc::new(text), large, lossy: false };
                t.rev = t.rev.wrapping_add(1);
                t.preview_rev = t.rev;
                format!("Saved {name}")
            }
            Err(e) => format!("Couldn't save {name}: {e}"),
        };
        self.show_toast(toast, now);
        // Only a successful write closes the tab: a failed save must keep the edits.
        if close && succeeded && self.tabs.iter().any(|t| t.id == tab) {
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
            t.rev = t.rev.wrapping_add(1);
            t.preview_rev = t.rev;
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
        t.rev = t.rev.wrapping_add(1);
        t.preview_rev = t.rev;
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
                    self.queue_scan(path);
                    return;
                }
                self.refresh_image(&path);
                for id in self.tab_ids_for(&path) {
                    self.jobs.push(Job::Load { tab: id, path: path.clone() });
                }
            }
            FileEvent::Removed(path) => {
                if self.is_root(&path) {
                    self.queue_scan(path);
                    return;
                }
                for id in self.tab_ids_for(&path) {
                    self.mark_deleted(id);
                }
            }
        }
    }

    /// Queues a root scan unless one is already waiting.
    fn queue_scan(&mut self, root: PathBuf) {
        let job = Job::Scan { root };
        if !self.jobs.contains(&job) {
            self.jobs.push(job);
        }
    }

    /// Lets the editor preview catch up with a burst of typing once it has been quiet long
    /// enough, so a fast typist does not re-parse the document on every keystroke.
    pub fn catch_up_preview(&mut self, now: f64, tx: &mut Tx) -> bool {
        let Some(tab) = self.tabs.active_mut() else { return false };
        if tab.preview_rev == tab.rev {
            return false;
        }
        let wait = PREVIEW_QUIET - (now - tab.last_edit);
        if wait > 0.0 {
            tx.wake_after(Duration::from_secs_f64(wait));
            return false;
        }
        tab.preview_rev = tab.rev;
        true
    }

    /// Queues a decode for each local image not yet cached or pending. Paths are already resolved
    /// on the worker.
    pub fn want_images(&mut self, paths: Vec<PathBuf>) {
        for p in paths {
            if self.images.pending_len() >= MAX_IN_FLIGHT_IMAGES {
                break;
            }
            if !self.images.has(&p) && !self.images.pending(&p) && !self.images.failed(&p) && !self.images.decoded(&p) {
                self.images.mark_pending(p.clone());
                self.jobs.push(Job::Image { key: p });
            }
        }
    }

    /// The local images a loaded document names, in document order. Only the active tab's window
    /// is queued now; the rest wait until it is scrolled to.
    pub fn load_images(&mut self, tab: u64, images: Vec<PathBuf>) {
        if let Some(t) = self.tabs.get_mut(tab) {
            t.images = images;
            t.image_window.clear();
        }
        self.want_visible_images(tab);
    }

    /// Queues the images around the active document's scroll position. Called each frame so the
    /// queue refills as the reader scrolls and as earlier decodes finish.
    pub fn want_visible_images(&mut self, tab: u64) {
        if self.tabs.active().map(|t| t.id) != Some(tab) {
            return;
        }
        let paths = {
            let Some(t) = self.tabs.get_mut(tab) else { return };
            let [offset, max, _, _] = t.scroll_at.get();
            let win = window(&t.images, crate::progress::fraction(offset, max));
            if win != t.image_window {
                for p in &win {
                    if !t.image_window.contains(p) {
                        self.images.clear_decoded(p);
                    }
                }
                t.image_window = win.clone();
            }
            win
        };
        self.want_images(paths);
    }

    /// Queues the editor preview's images around its scroll position.
    pub fn want_visible_edit_images(&mut self) {
        let paths = {
            let Some(t) = self.tabs.active_mut() else { return };
            let [offset, max] = self.preview_at.get();
            let win = window(&t.edit_images, crate::progress::fraction(offset, max));
            if win != t.edit_window {
                for p in &win {
                    if !t.edit_window.contains(p) {
                        self.images.clear_decoded(p);
                    }
                }
                t.edit_window = win.clone();
            }
            win
        };
        self.want_images(paths);
    }

    /// Runs once per frame: the image window the current mode shows.
    pub fn sync_visible_images(&mut self) {
        if matches!(self.mode, Mode::Edit) {
            self.want_visible_edit_images();
        } else if let Some(id) = self.tabs.active().map(|t| t.id) {
            self.want_visible_images(id);
        }
        if let Some(id) = self.tabs.active().map(|t| t.id) {
            self.want_web_images(id);
        }
    }

    /// A cached local image changed on disk: decodes the file again. The old picture stays on
    /// screen until the new one lands, and stays if the new one fails to decode.
    fn refresh_image(&mut self, path: &Path) {
        // A cached picture is re-decoded; one whose decode failed before the file existed is
        // retried now that it changed. A picture already queued is left alone.
        let cached = self.images.has(path);
        let pending = self.images.pending(path);
        let failed = self.images.failed(path);
        if (cached && !pending) || failed {
            self.images.clear_pending(path);
            self.images.clear_failed(path);
            self.images.clear_decoded(path);
            self.images.mark_pending(path.to_path_buf());
            self.jobs.push(Job::Image { key: path.to_path_buf() });
        }
    }

    /// Whether `tab`'s web images may be fetched: the setting is Always, or the reader pressed Load.
    pub fn web_allowed_for(&self, tab: u64) -> bool {
        self.settings.web_images == crate::settings::WebImages::Always || self.web_allowed.contains(&tab)
    }

    /// Queues a fetch for each of `urls` not cached or pending, when `tab` may fetch. Nothing
    /// leaves the machine otherwise.
    pub fn want_web(&mut self, tab: u64, urls: &[String]) {
        if !self.web_allowed_for(tab) {
            return;
        }
        for url in urls {
            if self.images.pending_len() >= MAX_PENDING_IMAGES {
                break;
            }
            let key = crate::images::web_key(url);
            if !self.images.has(&key) && !self.images.pending(&key) && !self.images.failed(&key) && !self.images.decoded(&key) {
                self.images.mark_pending(key);
                self.jobs.push(Job::WebImage { url: url.clone() });
            }
        }
    }

    /// A tab's document names these web images. Remembered for the banner, fetched if allowed.
    pub fn doc_web_images(&mut self, tab: u64, urls: Vec<String>) {
        if urls.is_empty() {
            self.web_urls.remove(&tab);
            return;
        }
        self.web_urls.insert(tab, urls);
        self.want_web_images(tab);
    }

    /// The Load button: allows `tab`'s web images and fetches the visible window.
    pub fn load_web_images(&mut self, tab: u64) {
        self.web_allowed.insert(tab);
        self.want_web_images(tab);
    }

    /// Queues the web images around the active tab's scroll position. Like local images, only a
    /// window is fetched; scrolling to the rest queues them.
    pub fn want_web_images(&mut self, tab: u64) {
        if self.tabs.active().map(|t| t.id) != Some(tab) {
            return;
        }
        let win = {
            let Some(t) = self.tabs.get_mut(tab) else { return };
            let Some(urls) = self.web_urls.get(&tab) else { return };
            let frac = if matches!(self.mode, Mode::Edit) {
                let [offset, max] = self.preview_at.get();
                crate::progress::fraction(offset, max)
            } else {
                let [offset, max, _, _] = t.scroll_at.get();
                crate::progress::fraction(offset, max)
            };
            let win = window(urls, frac);
            if win != t.web_window {
                for url in &win {
                    if !t.web_window.contains(url) {
                        self.images.clear_decoded(&crate::images::web_key(url));
                    }
                }
                t.web_window = win.clone();
            }
            win
        };
        self.want_web(tab, &win);
    }

    /// How many web images `tab` shows only as alt text because loading them is not allowed yet.
    pub fn blocked_web_images(&self, tab: u64) -> usize {
        if self.web_allowed_for(tab) { 0 } else { self.web_urls.get(&tab).map_or(0, Vec::len) }
    }

    /// Queues the images the active tab's edit buffer names, once per change of its text, so the
    /// Edit preview shows pictures as they are typed. Called each frame from `poll`.
    pub fn sync_edit_images(&mut self) {
        if !matches!(self.mode, Mode::Edit) {
            return;
        }
        let Some(tab) = self.tabs.active() else { return };
        let Some(buf) = tab.edit.as_ref() else { return };
        let seen = (tab.id, tab.preview_rev);
        if self.edit_images_seen == Some(seen) {
            return;
        }
        let (id, path) = (tab.id, tab.path.clone());
        let srcs = crate::images::image_srcs(crate::load::strip_frontmatter(&buf.text.text));
        self.edit_images_seen = Some(seen);
        let (local, mut web) = crate::images::split_srcs(&path, &srcs);
        web.sort();
        web.dedup();
        if let Some(t) = self.tabs.get_mut(id) {
            t.edit_images = local;
            t.edit_window.clear();
        }
        self.want_visible_edit_images();
        self.doc_web_images(id, web);
    }

    /// A worker finished decoding an image. A failure frees the slot and is remembered, so a
    /// broken picture is not retried until its file changes.
    pub fn image_ready(&mut self, key: PathBuf, image: Option<(rune::Image, usize)>) {
        match image {
            Some((img, bytes)) => self.images.insert(key, img, bytes),
            // A refresh that failed (a file caught mid-write) keeps the old picture and may be tried again.
            None if self.images.has(&key) => self.images.clear_pending(&key),
            None => self.images.mark_failed(key),
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
    pub fn scanned(&mut self, root: &Path, tree: Result<TreeModel<FileNode<PathBuf>>, String>, open: Option<PathBuf>) {
        let key = canonical(root);
        let Some(i) = self.sidebar.roots.iter().position(|r| r.path == key) else {
            return;
        };
        let open = match tree {
            Ok(tree) => {
                let r = &mut self.sidebar.roots[i];
                r.scanning = false;
                r.error = None;
                r.tree = tree;
                open
            }
            Err(e) => {
                let r = &mut self.sidebar.roots[i];
                r.scanning = false;
                r.error = Some(e);
                None
            }
        };
        if self.tabs.is_empty() && let Some(rel) = open {
            self.open_file(&key.join(rel));
        }
    }

    /// Queues a fresh load for the active tab and shows it as loading again. Try Again and
    /// Reload from Disk both use it.
    pub fn requeue_active_load(&mut self) {
        let Some(tab) = self.tabs.active_mut() else { return };
        if tab.edit.as_ref().is_some_and(EditBuffer::dirty) {
            // Reloading would rebase the saved hash under the edits, so the next save would
            // overwrite the file's new content without asking. Raise the conflict banner instead.
            tab.conflict = true;
            return;
        }
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

    pub(crate) fn set_zoom(&mut self, zoom: f32) {
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

/// The items to queue around `frac` (0 at the top, 1 at the bottom). A short list is taken
/// whole. A long one gets one window around the reader plus the first window, so scrolling back
/// to the top is instant; the cache evicts the rest as it fills.
fn window<T: Clone>(items: &[T], frac: f32) -> Vec<T> {
    let n = items.len();
    if n <= LAZY_IMAGE_WINDOW * 2 {
        return items.to_vec();
    }
    let center = ((frac * n as f32) as usize).min(n - 1);
    let lo = center.saturating_sub(LAZY_IMAGE_WINDOW);
    let hi = (center + LAZY_IMAGE_WINDOW).min(n);
    let mut out = items[lo..hi].to_vec();
    if lo >= LAZY_IMAGE_WINDOW {
        out.extend_from_slice(&items[..LAZY_IMAGE_WINDOW]);
    }
    out
}


#[cfg(test)]
#[path = "app_tests.rs"]
mod tests;
