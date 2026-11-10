//! Local images in a document (§5.3): resolving each `src` against the document's folder,
//! decoding off the UI thread, and a bounded cache of the decoded pictures.

use std::collections::{HashMap, HashSet};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use pulldown_cmark::{Event, Parser, Tag};

use crate::theme::READ_W_WIDE;

/// Most decoded pictures kept at once.
pub const CACHE_ENTRIES: usize = 64;
/// Most decoded RGBA bytes kept at once (128 MB).
pub const CACHE_BYTES: usize = 128 * 1024 * 1024;
/// Wider pictures are scaled down on decode, to twice the reading column (1920 px).
pub const MAX_DECODE_WIDTH: u32 = 2 * READ_W_WIDE as u32;
/// Largest allocation the decoder may make for one picture.
const MAX_DECODE_ALLOC: u64 = 256 * 1024 * 1024;

/// Where an image's `src` points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// A file on disk, absolute.
    Local(PathBuf),
    /// An `http:` or `https:` address. Never fetched without a click, and there is no HTTP client yet.
    // TODO(upstream): fetch on "Load" once an HTTP client is in the app; until then the placeholder stays.
    Remote(String),
    /// Empty, a fragment, or another scheme (`data:`, `file:`).
    Unsupported,
}

/// Resolves `src` as written in the document at `doc`. Relative paths are taken from the
/// document's folder; a query or fragment is dropped and `%XX` escapes are decoded.
pub fn resolve(doc: &Path, src: &str) -> Resolved {
    let src = src.trim();
    if src.is_empty() || src.starts_with('#') {
        return Resolved::Unsupported;
    }
    let lower = src.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") {
        return Resolved::Remote(src.to_string());
    }
    if has_scheme(src) {
        return Resolved::Unsupported;
    }
    let file = percent_decode(src.split(['?', '#']).next().unwrap_or_default());
    if file.is_empty() {
        return Resolved::Unsupported;
    }
    let path = Path::new(&file);
    if path.is_absolute() {
        Resolved::Local(path.to_path_buf())
    } else {
        Resolved::Local(doc.parent().unwrap_or(Path::new("")).join(path))
    }
}

/// True when the text starts with a URL scheme such as `tel:` (a colon before any slash).
pub fn has_scheme(url: &str) -> bool {
    match url.find(':') {
        Some(colon) => !url[..colon].contains('/'),
        None => false,
    }
}

/// `%XX` escapes decoded; an invalid escape is kept as written.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(hex) = s.get(i + 1..i + 3)
            && let Ok(b) = u8::from_str_radix(hex, 16)
        {
            out.push(b);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The `src` of every image in `src_md` (Markdown source), in document order. Images inside
/// code blocks are not images, so they are left out.
pub fn image_srcs(src_md: &str) -> Vec<String> {
    Parser::new(src_md)
        .filter_map(|event| match event {
            Event::Start(Tag::Image { dest_url, .. }) => Some(dest_url.to_string()),
            _ => None,
        })
        .collect()
}

/// The picture cache: decoded images by absolute path, bounded by entries and by decoded bytes,
/// least recently used first out. A path that was asked for and has no picture is `pending`:
/// queued, or failed, so it is not asked for again.
pub struct ImageCache {
    entries: HashMap<PathBuf, Entry>,
    pending: HashSet<PathBuf>,
    bytes: usize,
    tick: Cell<u64>,
}

struct Entry {
    image: rune::Image,
    bytes: usize,
    used: Cell<u64>,
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageCache {
    pub fn new() -> Self {
        Self { entries: HashMap::new(), pending: HashSet::new(), bytes: 0, tick: Cell::new(0) }
    }

    fn next_tick(&self) -> u64 {
        let t = self.tick.get() + 1;
        self.tick.set(t);
        t
    }

    /// The picture for `p`, if cached. Counts as a use for the LRU order.
    #[allow(dead_code)] // Read by the tests; the budget pass (Task 9) reads it too.
    pub fn get(&self, p: &Path) -> Option<rune::Image> {
        let e = self.entries.get(p)?;
        e.used.set(self.next_tick());
        Some(e.image.clone())
    }

    pub fn has(&self, p: &Path) -> bool {
        self.entries.contains_key(p)
    }

    /// Every cached picture, for a view to draw from. Built once per view, so it does not touch the LRU.
    pub fn hits(&self) -> Rc<HashMap<PathBuf, rune::Image>> {
        Rc::new(self.entries.iter().map(|(k, e)| (k.clone(), e.image.clone())).collect())
    }

    /// Stores a decoded picture of `bytes` decoded bytes, then evicts the least recently used
    /// entries (never this one) until both bounds hold again.
    pub fn insert(&mut self, p: PathBuf, img: rune::Image, bytes: usize) {
        self.pending.remove(&p);
        if let Some(old) = self.entries.remove(&p) {
            self.bytes -= old.bytes;
        }
        let used = self.next_tick();
        self.bytes += bytes;
        self.entries.insert(p.clone(), Entry { image: img, bytes, used: Cell::new(used) });
        while self.entries.len() > CACHE_ENTRIES || self.bytes > CACHE_BYTES {
            let victim = self.entries.iter().filter(|(k, _)| **k != p).min_by_key(|(_, e)| e.used.get()).map(|(k, _)| k.clone());
            let Some(victim) = victim else { break };
            if let Some(e) = self.entries.remove(&victim) {
                self.bytes -= e.bytes;
            }
        }
    }

    /// True when `p` was asked for and has no picture yet, or failed.
    pub fn pending(&self, p: &Path) -> bool {
        self.pending.contains(p)
    }

    pub fn mark_pending(&mut self, p: PathBuf) {
        self.pending.insert(p);
    }

    /// Decoded bytes held.
    #[allow(dead_code)] // For the memory budget (Task 9) and the tests.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// The shared `rune::ImageCache` that turns decoded RGBA into a `rune::Image`. Its decoder
/// threads do the mip chain; `decode` waits for them, so it needs no redraw callback.
fn shared_cache() -> &'static rune::ImageCache {
    static CACHE: OnceLock<rune::ImageCache> = OnceLock::new();
    CACHE.get_or_init(|| rune::ImageCache::new(Arc::new(|| {})))
}

/// Decodes the image file at `path` on the calling (worker) thread, scaled down to at most
/// `max_w` pixels wide. Returns the picture and its decoded size in bytes, or `None` when the
/// file can't be read, isn't an image, or is too big to keep.
pub fn decode(path: &Path, max_w: u32) -> Option<(rune::Image, usize)> {
    let path = path.to_path_buf();
    // A codec can panic on a malformed file; that fails this one image, not the worker.
    let rgba = catch_unwind(AssertUnwindSafe(move || -> Option<image::RgbaImage> {
        let mut reader = image::ImageReader::open(&path).ok()?.with_guessed_format().ok()?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(MAX_DECODE_ALLOC);
        reader.limits(limits);
        Some(reader.decode().ok()?.into_rgba8())
    }))
    .ok()??;
    let rgba = if rgba.width() > max_w {
        let h = (u64::from(rgba.height()) * u64::from(max_w) / u64::from(rgba.width())).max(1) as u32;
        image::imageops::resize(&rgba, max_w, h, image::imageops::FilterType::Triangle)
    } else {
        rgba
    };
    let (w, h) = rgba.dimensions();
    let bytes = w as usize * h as usize * 4;
    if bytes > CACHE_BYTES {
        return None;
    }
    let img = shared_cache().rgba(w, h, rgba.into_raw());
    img.wait().ok()?;
    Some((img, bytes))
}

#[cfg(test)]
#[path = "images_tests.rs"]
mod tests;
