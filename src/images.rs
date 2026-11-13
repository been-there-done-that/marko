//! Local images in a document (§5.3): resolving each `src` against the document's folder,
//! decoding off the UI thread, and a bounded cache of the decoded pictures.

use std::collections::{HashMap, HashSet};
use std::io::Read as _;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use pulldown_cmark::{Event, Parser, Tag};

use crate::theme::READ_W_WIDE;

/// Most decoded pictures kept at once.
pub const CACHE_ENTRIES: usize = 32;
/// Most decoded RGBA bytes kept at once (32 MB). `rune` keeps these as GPU textures, but on an
/// integrated GPU they are shared system memory, so this stays well inside the process budget.
pub const CACHE_BYTES: usize = 32 * 1024 * 1024;
/// Wider pictures are scaled down on decode, to twice the reading column (1920 px).
pub const MAX_DECODE_WIDTH: u32 = 2 * READ_W_WIDE as u32;
/// Largest allocation the decoder may make for one picture. Bounded so a huge photo cannot push
/// the process far past its memory budget. 64 MB is about a 16-megapixel image; larger ones are
/// refused before decoding, because `image`'s `max_alloc` is advisory and some decoders ignore it.
const MAX_DECODE_ALLOC: u64 = 64 * 1024 * 1024;
/// Largest SVG file read (a vector drawing has no business being larger).
const MAX_SVG_BYTES: u64 = 20 * 1024 * 1024;
/// Side, in points, an SVG with no readable size is drawn at.
const SVG_DEFAULT_PT: f32 = 640.0;

/// Where an image's `src` points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolved {
    /// A file on disk, absolute.
    Local(PathBuf),
    /// An `http:` or `https:` address. Fetched only when the Web images setting is Always or the
    /// reader pressed Load for the document (`AppState::want_web`).
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

/// Sorts image sources into the local files (resolved against the document at `doc`) and the web
/// addresses they name; anything else is dropped.
pub fn split_srcs(doc: &Path, srcs: &[String]) -> (Vec<PathBuf>, Vec<String>) {
    let (mut local, mut web) = (Vec::new(), Vec::new());
    for src in srcs {
        match resolve(doc, src) {
            Resolved::Local(p) => local.push(p),
            Resolved::Remote(url) => web.push(url),
            Resolved::Unsupported => {}
        }
    }
    (local, web)
}

/// The picture cache: decoded images by absolute path, bounded by entries and by decoded bytes,
/// least recently used first out. A path that was asked for and has no picture is `pending`:
/// queued, or failed, so it is not asked for again.
pub struct ImageCache {
    entries: HashMap<PathBuf, Entry>,
    pending: HashSet<PathBuf>,
    bytes: usize,
    tick: Cell<u64>,
    /// Bumped whenever the decoded set changes, so a memoised view knows to rebuild.
    version: u64,
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
        Self { entries: HashMap::new(), pending: HashSet::new(), bytes: 0, tick: Cell::new(0), version: 0 }
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
        self.version = self.version.wrapping_add(1);
    }

    /// Bumped whenever a picture is added or evicted.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// How many pictures are queued or failed, for the fetch cap.
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// The local files whose pictures are cached, for the watcher to follow.
    pub fn local_paths(&self) -> Vec<PathBuf> {
        self.entries.keys().filter(|p| p.is_absolute()).cloned().collect()
    }

    /// True when `p` was asked for and has no picture yet, or failed.
    pub fn pending(&self, p: &Path) -> bool {
        self.pending.contains(p)
    }

    /// Forgets that `p` was asked for, so a failed refresh of a picture we still hold can be retried.
    pub fn clear_pending(&mut self, p: &Path) {
        self.pending.remove(p);
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

/// The cache key a fetched web image is stored under. Not an absolute path, so it never meets a local file.
pub fn web_key(url: &str) -> PathBuf {
    PathBuf::from(format!("web:{}", url.trim()))
}

/// Decodes the image file at `path` on the calling (worker) thread, scaled down to at most
/// `max_w` pixels wide. SVG files (`.svg`, `.svgz`) are drawn at their own size. Returns the
/// picture and its decoded size in bytes, or `None` when the file can't be read, isn't an
/// image, or is too big to keep.
pub fn decode(path: &Path, max_w: u32) -> Option<(rune::Image, usize)> {
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    if matches!(ext.as_deref(), Some("svg" | "svgz")) {
        let meta = std::fs::metadata(path).ok()?;
        if meta.len() > MAX_SVG_BYTES {
            return None;
        }
        return decode_svg(std::fs::read(path).ok()?);
    }
    let path = path.to_path_buf();
    decode_with(move || {
        if let Some(img) = fast_jpeg_file(&path, max_w) {
            return Some(img);
        }
        let reader = image::ImageReader::open(&path).ok()?;
        decode_reader(reader)
    }, max_w)
}

/// Decodes an image held in memory (a fetched web image), as `decode` does a file.
pub fn decode_bytes(bytes: &[u8], max_w: u32) -> Option<(rune::Image, usize)> {
    if looks_like_svg(bytes) {
        return decode_svg(bytes.to_vec());
    }
    if let Some(img) = fast_jpeg_bytes(bytes, max_w) {
        return decode_with(move || Some(img), max_w);
    }
    let bytes = std::io::Cursor::new(bytes);
    decode_with(move || decode_reader(image::ImageReader::new(bytes)), max_w)
}

fn fast_jpeg_file(path: &Path, max_w: u32) -> Option<image::DynamicImage> {
    if !is_jpeg(path) {
        return None;
    }
    decode_jpeg(std::io::BufReader::new(std::fs::File::open(path).ok()?), max_w)
}

fn fast_jpeg_bytes(bytes: &[u8], max_w: u32) -> Option<image::DynamicImage> {
    if !bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return None;
    }
    decode_jpeg(std::io::Cursor::new(bytes), max_w)
}

/// True when the file is a JPEG, by extension or by its `FF D8 FF` magic. A misnamed file still
/// takes the fast path when its bytes are a JPEG.
fn is_jpeg(path: &Path) -> bool {
    let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase());
    if matches!(ext.as_deref(), Some("jpg" | "jpeg" | "jfif")) {
        return true;
    }
    let mut magic = [0u8; 3];
    std::fs::File::open(path).and_then(|mut f| f.read_exact(&mut magic)).is_ok()
        && magic == [0xFF, 0xD8, 0xFF]
}

/// Decodes a JPEG with `jpeg-decoder`, which can ask the DCT for a half, quarter or eighth-scale
/// output. That keeps the transient buffer for a large photo far below a full-resolution decode.
/// Returns `None` for 16-bit or CMYK JPEGs, lossless JPEG, or any error; the caller falls back to
/// `image`, which handles those.
fn decode_jpeg<R: std::io::Read>(reader: R, max_w: u32) -> Option<image::DynamicImage> {
    use jpeg_decoder::{CodingProcess, PixelFormat};

    let mut dec = jpeg_decoder::Decoder::new(reader);
    dec.set_max_decoding_buffer_size(MAX_DECODE_ALLOC as usize * 2);
    dec.read_info().ok()?;
    let info = dec.info()?;
    if !matches!(info.coding_process, CodingProcess::DctSequential | CodingProcess::DctProgressive) {
        return None;
    }
    let (w, h) = (u32::from(info.width), u32::from(info.height));
    if w == 0 || h == 0 {
        return None;
    }
    let (tw, th) = target_dims(w, h, max_w, CACHE_BYTES);
    let (ow, oh) = dec.scale(tw as u16, th as u16).ok()?;
    let (ow, oh) = (u32::from(ow), u32::from(oh));
    // Check the scaled output, not the full image: a 100 MP photo at 1/4 is small.
    if too_big(ow, oh) {
        return None;
    }
    let pixels = dec.decode().ok()?;
    match info.pixel_format {
        PixelFormat::RGB24 => Some(image::DynamicImage::ImageRgb8(image::RgbImage::from_raw(ow, oh, pixels)?)),
        PixelFormat::L8 => Some(image::DynamicImage::ImageLuma8(image::GrayImage::from_raw(ow, oh, pixels)?)),
        _ => None,
    }
}

/// Reads the header first, refuses a picture whose decoded RGBA would exceed the budget, and only
/// then decodes it. The dimensions are strict, so a malformed header cannot make a codec allocate
/// far more than the limits allow.
fn decode_reader<R: std::io::BufRead + std::io::Seek>(reader: image::ImageReader<R>) -> Option<image::DynamicImage> {
    use image::ImageDecoder as _;

    let mut reader = reader.with_guessed_format().ok()?;
    limit(&mut reader);
    let decoder = reader.into_decoder().ok()?;
    let (w, h) = decoder.dimensions();
    if too_big(w, h) {
        return None;
    }
    image::DynamicImage::from_decoder(decoder).ok()
}

/// Whether a picture's decoded RGBA would exceed the per-image budget. Checked from the header so
/// the decoder never allocates it.
fn too_big(w: u32, h: u32) -> bool {
    u64::from(w) * u64::from(h) * 4 > MAX_DECODE_ALLOC
}

/// Largest width or height a decoder may report. The pixel product is checked separately; this
/// rejects absurd headers before a codec can trust them.
const MAX_DECODE_SIDE: u32 = 16_384;

fn limit<R: std::io::BufRead + std::io::Seek>(reader: &mut image::ImageReader<R>) {
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(MAX_DECODE_ALLOC);
    // Strict dimension caps as well: `max_alloc` is advisory for some decoders.
    limits.max_image_width = Some(MAX_DECODE_SIDE);
    limits.max_image_height = Some(MAX_DECODE_SIDE);
    reader.limits(limits);
}

fn decode_with(read: impl FnOnce() -> Option<image::DynamicImage> + std::panic::UnwindSafe, max_w: u32) -> Option<(rune::Image, usize)> {
    // A codec can panic on a malformed file; that fails this one image, not the worker.
    let rgba = catch_unwind(move || read().map(image::DynamicImage::into_rgba8)).ok()??;
    let rgba = fit(rgba, max_w);
    let (w, h) = rgba.dimensions();
    let bytes = w as usize * h as usize * 4;
    if bytes > CACHE_BYTES {
        return None;
    }
    let img = shared_cache().rgba(w, h, rgba.into_raw());
    img.wait().ok()?;
    Some((img, bytes))
}

/// Scales a decoded picture down so it fits both the reading column and the cache, keeping its
/// aspect ratio. Width is capped at `max_w`; if the result would still exceed the cache, it is
/// scaled further. This also catches a tall, narrow image, which a width-only cap would keep at
/// full size until the cache rejected it after decoding.
fn fit(rgba: image::RgbaImage, max_w: u32) -> image::RgbaImage {
    fit_with(rgba, max_w, CACHE_BYTES)
}

fn fit_with(rgba: image::RgbaImage, max_w: u32, max_bytes: usize) -> image::RgbaImage {
    let (w, h) = rgba.dimensions();
    let (nw, nh) = target_dims(w, h, max_w, max_bytes);
    if (nw, nh) == (w, h) {
        return rgba;
    }
    image::imageops::resize(&rgba, nw, nh, image::imageops::FilterType::Triangle)
}

/// The size a picture should be reduced to so it fits both the reading column and the cache,
/// keeping its aspect ratio. Shared by `fit` and the JPEG fast path, which asks the decoder for a
/// DCT-scaled output near this size.
fn target_dims(w: u32, h: u32, max_w: u32, max_bytes: usize) -> (u32, u32) {
    let pixels = u64::from(w) * u64::from(h);
    let max_pixels = (max_bytes / 4) as u64;
    if w <= max_w && pixels <= max_pixels {
        return (w, h);
    }
    let by_width = f64::from(max_w) / f64::from(w);
    let by_pixels = (max_pixels as f64 / pixels as f64).sqrt();
    let scale = by_width.min(by_pixels).min(1.0);
    let mut nw = ((f64::from(w) * scale).floor() as u32).max(1).min(w).min(max_w.max(1));
    let mut nh = ((f64::from(h) * scale).floor() as u32).max(1).min(h);
    // Floor can still land one or two pixels over after rounding; shave the longer side.
    while u64::from(nw) * u64::from(nh) > max_pixels && (nw > 1 || nh > 1) {
        if nw >= nh && nw > 1 {
            nw -= 1;
        } else if nh > 1 {
            nh -= 1;
        } else {
            break;
        }
    }
    (nw, nh)
}

/// Largest an SVG may be after gzip decompression (32 MB), matching the rasteriser's own cap.
const MAX_SVG_DECOMPRESSED: u64 = 32 * 1024 * 1024;

/// Gunzips `bytes` but never produces more than `cap` bytes, so a small archive cannot expand
/// into an unbounded allocation.
fn bounded_gunzip(bytes: &[u8], cap: u64) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes).take(cap + 1).read_to_end(&mut out).ok()?;
    (out.len() as u64 <= cap).then_some(out)
}

/// True when the drawing names no external file: `<image>` and `<feImage>` may only carry
/// `data:` URIs. usvg's default resolver otherwise calls `std::fs::read` on whatever path an
/// `href` names, so a malicious drawing could block on a FIFO or read `/dev/zero` until memory
/// runs out. Anything that does not parse as XML is refused too.
fn svg_is_self_contained(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else { return false };
    let Ok(doc) = roxmltree::Document::parse(text) else { return false };
    doc.descendants()
        .filter(|n| matches!(n.tag_name().name(), "image" | "feImage"))
        .all(|n| {
            n.attributes()
                .filter(|a| a.name() == "href")
                .all(|a| a.value().trim_start().to_ascii_lowercase().starts_with("data:"))
        })
}

/// Rasterises SVG (or gzipped SVG) bytes at the drawing's own size, no wider than the wide
/// reading column.
fn decode_svg(bytes: Vec<u8>) -> Option<(rune::Image, usize)> {
    let bytes = if bytes.starts_with(&[0x1f, 0x8b]) { bounded_gunzip(&bytes, MAX_SVG_DECOMPRESSED)? } else { bytes };
    if !svg_is_self_contained(&bytes) {
        return None;
    }
    let side = svg_size(&bytes).map_or(SVG_DEFAULT_PT, |(w, h)| w.max(h)).clamp(1.0, MAX_DECODE_WIDTH as f32);
    let img = catch_unwind(AssertUnwindSafe(|| shared_cache().svg(bytes, side, 1.0))).ok()?;
    let [w, h] = img.wait().ok()?;
    let bytes = w as usize * h as usize * 4;
    (bytes <= CACHE_BYTES).then_some((img, bytes))
}

/// Bytes that are SVG text, or gzip (SVGZ); a raster format never starts either way.
fn looks_like_svg(bytes: &[u8]) -> bool {
    if bytes.starts_with(&[0x1f, 0x8b]) {
        return true;
    }
    let head = &bytes[..bytes.len().min(2048)];
    let text = String::from_utf8_lossy(head);
    text.trim_start().starts_with('<') && text.contains("<svg")
}

/// The drawing's own size in points, from the root tag's `width` and `height`, or its `viewBox`.
/// None for gzip, percentages, or a tag without sizes.
fn svg_size(bytes: &[u8]) -> Option<(f32, f32)> {
    let head = String::from_utf8_lossy(&bytes[..bytes.len().min(4096)]);
    let start = head.find("<svg")?;
    let tag = &head[start..start + head[start..].find('>')?];
    let attr = |name: &str| -> Option<String> {
        let mut from = 0;
        while let Some(i) = tag[from..].find(name) {
            let at = from + i;
            from = at + name.len();
            let before_ok = tag[..at].ends_with(char::is_whitespace);
            let rest = tag[from..].trim_start();
            if before_ok && let Some(rest) = rest.strip_prefix('=') {
                let rest = rest.trim_start();
                let quote = rest.chars().next().filter(|c| *c == '"' || *c == '\'')?;
                return rest[1..].split(quote).next().map(str::to_string);
            }
        }
        None
    };
    let number = |v: &str| -> Option<f32> {
        let v = v.trim();
        if v.ends_with('%') {
            return None;
        }
        let digits = v.trim_end_matches(|c: char| c.is_ascii_alphabetic());
        digits.parse::<f32>().ok().filter(|n| n.is_finite() && *n > 0.0)
    };
    if let (Some(w), Some(h)) = (attr("width").as_deref().and_then(number), attr("height").as_deref().and_then(number)) {
        return Some((w, h));
    }
    let vb = attr("viewBox")?;
    let parts: Vec<f32> = vb.split([' ', ',']).filter(|p| !p.is_empty()).filter_map(|p| p.parse().ok()).collect();
    match parts[..] {
        [_, _, w, h] if w > 0.0 && h > 0.0 => Some((w, h)),
        _ => None,
    }
}

#[cfg(test)]
#[path = "images_tests.rs"]
mod tests;
