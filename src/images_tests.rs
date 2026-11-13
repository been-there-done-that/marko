use super::*;
use std::path::Path;

fn tiny() -> rune::Image {
    shared_cache().rgba(1, 1, vec![0, 0, 0, 255])
}

#[test]
fn resolve_local_remote_and_bad() {
    let doc = Path::new("/d/doc.md");
    assert_eq!(resolve(doc, "pic.png"), Resolved::Local(PathBuf::from("/d/pic.png")));
    assert_eq!(resolve(doc, "img%20x.png"), Resolved::Local(PathBuf::from("/d/img x.png")));
    assert_eq!(resolve(doc, "pic.png?v=2"), Resolved::Local(PathBuf::from("/d/pic.png")));
    assert_eq!(resolve(doc, "/abs.png"), Resolved::Local(PathBuf::from("/abs.png")));
    assert_eq!(resolve(doc, "https://h.example/x.png"), Resolved::Remote("https://h.example/x.png".into()));
    assert_eq!(resolve(doc, ""), Resolved::Unsupported);
    assert_eq!(resolve(doc, "#top"), Resolved::Unsupported);
    assert_eq!(resolve(doc, "data:image/png;base64,AA"), Resolved::Unsupported);
}

#[test]
fn image_srcs_skips_code_blocks() {
    let src = "![a](x.png)\n\n```\n![b](y.png)\n```\n\n> ![c](z.png)";
    assert_eq!(image_srcs(src), vec!["x.png".to_string(), "z.png".to_string()]);
}

#[test]
fn cache_evicts_by_bytes() {
    let mut c = ImageCache::new();
    let part = CACHE_BYTES / 3;
    for name in ["/a.png", "/b.png", "/c.png", "/d.png"] {
        c.insert(PathBuf::from(name), tiny(), part);
    }
    assert!(c.bytes() <= CACHE_BYTES);
    assert!(!c.has(Path::new("/a.png")), "the least recently used goes first");
    assert!(c.has(Path::new("/d.png")));
}

#[test]
fn cache_evicts_by_count() {
    let mut c = ImageCache::new();
    for i in 0..=CACHE_ENTRIES {
        c.insert(PathBuf::from(format!("/{i}.png")), tiny(), 4);
    }
    assert_eq!(c.len(), CACHE_ENTRIES);
    assert!(!c.has(Path::new("/0.png")));
    let last = PathBuf::from(format!("/{CACHE_ENTRIES}.png"));
    assert!(c.get(&last).is_some());
}

#[test]
fn a_decoded_path_can_be_forgotten() {
    let mut c = ImageCache::new();
    let p = PathBuf::from("/x.png");
    c.insert(p.clone(), tiny(), 4);
    assert!(c.decoded(&p));
    c.clear_decoded(&p);
    assert!(!c.decoded(&p), "a window change makes the path queue again");
}

#[test]
fn bad_image_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.png");
    std::fs::write(&bad, b"not a png at all").unwrap();
    assert!(decode(&bad, MAX_DECODE_WIDTH).is_none());
    assert!(decode(&dir.path().join("missing.png"), MAX_DECODE_WIDTH).is_none());
}

#[test]
fn decode_downscales_wide_images() {
    let dir = tempfile::tempdir().unwrap();
    let wide = dir.path().join("wide.png");
    image::RgbaImage::new(3000, 10).save(&wide).unwrap();
    let (img, bytes) = decode(&wide, MAX_DECODE_WIDTH).expect("a valid PNG decodes");
    assert_eq!(img.size(), Some([1920, 6]));
    assert_eq!(bytes, 1920 * 6 * 4);
}

#[test]
fn huge_dimensions_are_refused_before_decoding() {
    assert!(too_big(100_000, 100_000));
    assert!(too_big(8_192, 8_192), "256 MB of RGBA is over the 64 MB budget");
    assert!(!too_big(4_096, 4_096), "64 MB is exactly at the budget");
    assert!(!too_big(1_920, 1_080));
}

#[test]
fn fit_leaves_images_that_already_fit_alone() {
    let fitted = fit_with(image::RgbaImage::new(800, 600), MAX_DECODE_WIDTH, CACHE_BYTES);
    assert_eq!(fitted.dimensions(), (800, 600));
}

#[test]
fn fit_scales_a_tall_image_to_the_cache() {
    // 100 x 100 is 40 KB; a 16 KB budget must shrink it rather than refusing it after decoding.
    let fitted = fit_with(image::RgbaImage::new(100, 100), MAX_DECODE_WIDTH, 16_000);
    let (w, h) = fitted.dimensions();
    assert!(w < 100 && h < 100, "both sides shrink, keeping the aspect ratio");
    assert!(u64::from(w) * u64::from(h) * 4 <= 16_000 + 4 * 100, "rounding slack only");
}

#[test]
fn target_dims_fit_or_scale() {
    assert_eq!(target_dims(800, 600, MAX_DECODE_WIDTH, CACHE_BYTES), (800, 600));
    assert_eq!(target_dims(4000, 3000, MAX_DECODE_WIDTH, CACHE_BYTES), (1920, 1440));
    let (w, h) = target_dims(100, 100_000, MAX_DECODE_WIDTH, 1_000_000);
    assert!(u64::from(w) * u64::from(h) <= 250_000, "the pixel budget is respected");
    assert!(h < 100_000);
}

#[test]
fn jpeg_fast_path_uses_dct_scaling() {
    let src = image::RgbImage::from_pixel(1600, 1200, image::Rgb([200, 30, 30]));
    let mut jpg = Vec::new();
    src.write_to(&mut std::io::Cursor::new(&mut jpg), image::ImageFormat::Jpeg).unwrap();
    let img = decode_jpeg(std::io::Cursor::new(jpg.as_slice()), 800).expect("a JPEG decodes");
    assert_eq!((img.width(), img.height()), (800, 600), "half-scale DCT, not a full decode");
}

#[test]
fn decoder_uses_the_jpeg_fast_path() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("photo.jpg");
    image::RgbImage::from_pixel(1600, 1200, image::Rgb([10, 120, 200])).save(&path).unwrap();
    let (img, bytes) = decode(&path, 800).expect("a JPEG decodes");
    assert_eq!(img.size(), Some([800, 600]));
    assert_eq!(bytes, 800 * 600 * 4);
}

#[test]
fn grayscale_jpeg_decodes() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("gray.jpg");
    image::GrayImage::from_pixel(400, 300, image::Luma([128])).save(&path).unwrap();
    let (img, _) = decode(&path, MAX_DECODE_WIDTH).expect("a grayscale JPEG decodes");
    assert_eq!(img.size(), Some([400, 300]));
}

#[test]
fn a_header_only_huge_png_is_refused() {
    // A PNG signature + IHDR claiming 100000 x 100000, with no IDAT. The dimensions are read
    // from the header, so the picture is refused before any pixel buffer exists.
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = b"IHDR".to_vec();
    ihdr.extend_from_slice(&100_000u32.to_be_bytes());
    ihdr.extend_from_slice(&100_000u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
    png.extend_from_slice(&13u32.to_be_bytes());
    png.extend_from_slice(&ihdr);
    png.extend_from_slice(&crc32(&ihdr).to_be_bytes());
    assert!(decode_bytes(&png, MAX_DECODE_WIDTH).is_none());
}

#[test]
fn a_corrupt_jpeg_is_refused_by_both_paths() {
    // SOI + a truncated APP0 header: the fast path cannot read the info, and `image` cannot
    // construct a decoder, so the picture is refused rather than half-drawn.
    let jpg = [0xFF, 0xD8, 0xFF, 0xE0, 0, 0];
    assert!(decode_bytes(&jpg, MAX_DECODE_WIDTH).is_none());
}

fn crc32(data: &[u8]) -> u32 {
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c ^= u32::from(b);
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
    }
    !c
}

#[test]
fn code_block_has_copy_button() {
    use crate::app::AppState;
    use crate::ui::view;
    use crate::worker::run_job;
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("code.md");
    std::fs::write(&doc, "# Code\n\n```rust\nfn main() {}\n```\n").unwrap();
    let mut s = AppState::new();
    s.open_paths(std::slice::from_ref(&doc), 0.0);
    for job in std::mem::take(&mut s.jobs) {
        run_job(job).apply(&mut s);
    }
    let mut rt = rune::Runtime::new(s, view, (1100.0, 760.0), rune::Env { system_dark: false, scale: 2.0 });
    rt.frame(1.0 / 60.0);
    assert!(rt.find("code-copy-0").is_some());
}

const SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="60" viewBox="0 0 120 60"><rect width="120" height="60" fill="#e33"/></svg>"##;

#[test]
fn svg_file_decodes_at_its_own_size() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("logo.svg");
    std::fs::write(&f, SVG).unwrap();
    let (img, bytes) = decode(&f, MAX_DECODE_WIDTH).expect("an SVG decodes");
    assert_eq!(img.size(), Some([120, 60]));
    assert_eq!(bytes, 120 * 60 * 4);
}

#[test]
fn svg_bytes_are_sniffed() {
    let (img, _) = decode_bytes(SVG.as_bytes(), MAX_DECODE_WIDTH).expect("SVG bytes decode");
    assert_eq!(img.size(), Some([120, 60]));
    assert!(decode_bytes(b"<html>not an image</html>", MAX_DECODE_WIDTH).is_none());
}

#[test]
fn svg_size_reads_attributes_or_viewbox() {
    assert_eq!(svg_size(SVG.as_bytes()), Some((120.0, 60.0)));
    assert_eq!(svg_size(br#"<svg viewBox="0 0 30 20" width="100%">"#), Some((30.0, 20.0)));
    assert_eq!(svg_size(br#"<svg width='10px' height='5px'>"#), Some((10.0, 5.0)));
    assert_eq!(svg_size(b"<svg>"), None);
}

#[test]
fn bad_svg_is_none() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("bad.svg");
    std::fs::write(&f, "<svg this is not xml").unwrap();
    assert!(decode(&f, MAX_DECODE_WIDTH).is_none());
}

#[test]
fn png_bytes_decode_in_memory() {
    let mut png = Vec::new();
    image::RgbaImage::new(8, 4).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    let (img, bytes) = decode_bytes(&png, MAX_DECODE_WIDTH).expect("PNG bytes decode");
    assert_eq!(img.size(), Some([8, 4]));
    assert_eq!(bytes, 8 * 4 * 4);
    assert!(decode_bytes(b"garbage", MAX_DECODE_WIDTH).is_none());
}

#[test]
fn web_key_is_never_a_local_path() {
    assert!(!web_key("https://h.example/x.png").is_absolute());
    assert_ne!(web_key("https://h.example/a.png"), web_key("https://h.example/b.png"));
}

// App-level behaviour: the web image policy, refresh on change, the editor preview, the lightbox.

mod flow {
    use super::*;
    use crate::app::{AppState, Job, Mode};
    use crate::settings::WebImages;
    use crate::ui::{editor, view};
    use crate::worker::run_job;
    use rune::{Env, Key, KeyEvent, Mods, Runtime};

    fn settle(s: &mut AppState) {
        while !s.jobs.is_empty() {
            for job in std::mem::take(&mut s.jobs) {
                run_job(job).apply(s);
            }
        }
    }

    /// A tempdir holding `doc.md` with `body`, opened and loaded, with its load jobs run.
    /// Web jobs are never run: these tests must not touch the network.
    fn open(body: &str) -> (tempfile::TempDir, AppState) {
        let dir = tempfile::tempdir().unwrap();
        let doc = crate::paths::canonical(dir.path()).join("doc.md");
        std::fs::write(&doc, body).unwrap();
        let mut s = AppState::new();
        s.open_paths(std::slice::from_ref(&doc), 0.0);
        for job in std::mem::take(&mut s.jobs) {
            run_job(job).apply(&mut s);
        }
        (dir, s)
    }

    fn web_jobs(s: &AppState) -> usize {
        s.jobs.iter().filter(|j| matches!(j, Job::WebImage { .. })).count()
    }

    fn runtime(s: AppState) -> Runtime<AppState> {
        let mut rt = Runtime::new(s, view, (1100.0, 760.0), Env { system_dark: false, scale: 2.0 });
        rt.on_key = Some(crate::ui::keys::on_key);
        rt.frame(1.0 / 60.0);
        rt
    }

    const TWO_WEB: &str = "![a](https://h.example/a.png)\n\n![b](https://h.example/b.png)\n\n![a again](https://h.example/a.png)\n";

    #[test]
    fn ask_shows_a_banner_and_fetches_nothing() {
        let (_d, s) = open(TWO_WEB);
        assert_eq!(web_jobs(&s), 0, "no fetch under Ask");
        let id = s.tabs.active().unwrap().id;
        assert_eq!(s.blocked_web_images(id), 2, "the same address twice counts once");
        let rt = runtime(s);
        assert!(rt.find("doc-banner-web").is_some());
        assert!(rt.find("doc-web-load").is_some());
    }

    #[test]
    fn load_allows_that_tab_and_queues_the_fetches() {
        let (_d, s) = open(TWO_WEB);
        let mut rt = runtime(s);
        let (x, y) = rt.find("doc-web-load").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert_eq!(web_jobs(&rt.state), 2);
        rt.frame(1.0 / 60.0);
        assert!(rt.find("doc-banner-web").is_none(), "the banner goes once loading is allowed");
        // A second Load (or a reload of the document) queues nothing twice.
        let id = rt.state.tabs.active().unwrap().id;
        rt.state.load_web_images(id);
        assert_eq!(web_jobs(&rt.state), 2);
    }

    #[test]
    fn always_queues_on_open_and_has_no_banner() {
        let dir = tempfile::tempdir().unwrap();
        let doc = crate::paths::canonical(dir.path()).join("doc.md");
        std::fs::write(&doc, TWO_WEB).unwrap();
        let mut s = AppState::new();
        s.settings.web_images = WebImages::Always;
        s.open_paths(std::slice::from_ref(&doc), 0.0);
        for job in std::mem::take(&mut s.jobs) {
            run_job(job).apply(&mut s);
        }
        assert_eq!(web_jobs(&s), 2);
        let rt = runtime(s);
        assert!(rt.find("doc-banner-web").is_none());
    }

    #[test]
    fn a_document_without_web_images_has_no_banner() {
        let (_d, s) = open("# plain\n\n![local](pic.png)\n");
        let rt = runtime(s);
        assert!(rt.find("doc-banner-web").is_none());
    }

    #[test]
    fn a_changed_image_file_is_decoded_again() {
        let (d, mut s) = open("![p](pic.png)\n");
        let pic = crate::paths::canonical(d.path()).join("pic.png");
        image::RgbaImage::new(4, 4).save(&pic).unwrap();
        s.want_images(vec![pic.clone()]);
        settle(&mut s);
        assert!(s.images.has(&pic));
        image::RgbaImage::new(8, 2).save(&pic).unwrap();
        s.file_event(crate::watch::FileEvent::Changed(pic.clone()));
        assert!(s.jobs.contains(&Job::Image { key: pic.clone() }));
        assert!(s.images.has(&pic), "the old picture stays until the new one lands");
        settle(&mut s);
        assert_eq!(s.images.get(&pic).unwrap().size(), Some([8, 2]));
        // A file never asked for is not decoded because it changed.
        s.file_event(crate::watch::FileEvent::Changed(d.path().join("other.png")));
        assert!(s.jobs.is_empty());
    }

    #[test]
    fn editor_preview_queues_images_as_they_are_typed() {
        let (d, mut s) = open("# t\n");
        let pic = crate::paths::canonical(d.path()).join("new.png");
        image::RgbaImage::new(4, 4).save(&pic).unwrap();
        assert!(editor::enter(&mut s));
        assert!(matches!(s.mode, Mode::Edit));
        s.sync_edit_images();
        assert!(s.jobs.is_empty());
        let buf = s.tabs.active_mut().unwrap().edit.as_mut().unwrap();
        let end = buf.text.text.len();
        buf.text.set_caret(end);
        buf.text.insert("\n![n](new.png)\n![w](https://h.example/w.png)\n");
        // The input callback bumps the revision on every edit; do the same here, then let the
        // preview settle so the image scan sees it.
        let tab = s.tabs.active_mut().unwrap();
        tab.rev += 1;
        tab.preview_rev = tab.rev;
        s.sync_edit_images();
        assert!(s.jobs.contains(&Job::Image { key: pic.clone() }));
        assert_eq!(web_jobs(&s), 0, "Ask still holds in the editor");
        // The same text queues nothing more.
        let n = s.jobs.len();
        s.sync_edit_images();
        assert_eq!(s.jobs.len(), n);
    }

    #[test]
    fn pressing_an_image_opens_the_lightbox_and_escape_closes_it() {
        let (d, mut s) = open("Some text.\n\n![the pic](pic.png)\n\nMore.\n");
        let pic = crate::paths::canonical(d.path()).join("pic.png");
        image::RgbaImage::from_pixel(400, 300, image::Rgba([200, 0, 0, 255])).save(&pic).unwrap();
        s.want_images(vec![pic.clone()]);
        settle(&mut s);
        assert!(s.images.has(&pic));
        let mut rt = runtime(s);
        for _ in 0..4 {
            rt.frame(1.0 / 60.0);
        }
        let (x, y) = rt.find("doc-image").expect("the decoded image is on screen");
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        rt.frame(1.0 / 60.0);
        assert_eq!(rt.state.lightbox.as_ref().map(|l| l.alt.as_str()), Some("the pic"));
        assert!(rt.find("lightbox").is_some());
        rt.key(KeyEvent { key: Key::Escape, mods: Mods::default() });
        rt.frame(1.0 / 60.0);
        assert!(rt.state.lightbox.is_none());
        rt.frame(1.0 / 60.0);
        assert!(rt.find("lightbox").is_none());
    }

    #[test]
    fn pressing_the_scrim_closes_the_lightbox() {
        let (_d, mut s) = open("# t\n");
        s.lightbox = Some(crate::app::Lightbox { image: tiny(), alt: String::new() });
        let mut rt = runtime(s);
        let (x, y) = rt.find("lightbox").unwrap();
        rt.pointer_down(x, y);
        rt.pointer_up(x, y);
        rt.frame(1.0 / 60.0);
        assert!(rt.state.lightbox.is_none());
    }
}

#[test]
fn svg_external_hrefs_are_refused() {
    assert!(!svg_is_self_contained(br#"<svg xmlns="http://www.w3.org/2000/svg"><image href="/dev/zero"/></svg>"#));
    assert!(!svg_is_self_contained(
        br#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><image xlink:href="pic.png"/></svg>"#
    ));
    assert!(!svg_is_self_contained(
        br#"<svg xmlns="http://www.w3.org/2000/svg"><filter><feImage href="/etc/passwd"/></filter></svg>"#
    ));
    assert!(!svg_is_self_contained(b"not xml at all"));
}

#[test]
fn svg_data_hrefs_and_plain_shapes_are_allowed() {
    assert!(svg_is_self_contained(
        br#"<svg xmlns="http://www.w3.org/2000/svg"><image href="data:image/png;base64,AA=="/></svg>"#
    ));
    assert!(svg_is_self_contained(br#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="1" height="1"/></svg>"#));
}

#[test]
fn gzip_is_bounded() {
    use std::io::Write as _;
    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    enc.write_all(&vec![b'a'; 4096]).unwrap();
    let gz = enc.finish().unwrap();
    assert_eq!(bounded_gunzip(&gz, 8192).unwrap().len(), 4096);
    assert!(bounded_gunzip(&gz, 1024).is_none(), "a cap smaller than the payload refuses it");
}
