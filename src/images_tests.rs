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
    let mb50 = 50 * 1024 * 1024;
    c.insert(PathBuf::from("/a.png"), tiny(), mb50);
    c.insert(PathBuf::from("/b.png"), tiny(), mb50);
    c.insert(PathBuf::from("/c.png"), tiny(), mb50);
    assert!(c.bytes() <= CACHE_BYTES);
    assert!(!c.has(Path::new("/a.png")), "the least recently used goes first");
    assert!(c.has(Path::new("/c.png")));
}

#[test]
fn cache_evicts_by_count() {
    let mut c = ImageCache::new();
    for i in 0..=CACHE_ENTRIES {
        c.insert(PathBuf::from(format!("/{i}.png")), tiny(), 4);
    }
    assert_eq!(c.len(), CACHE_ENTRIES);
    assert!(!c.has(Path::new("/0.png")));
    assert!(c.get(Path::new("/64.png")).is_some());
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
fn code_block_has_copy_button() {
    use crate::app::AppState;
    use crate::ui::view;
    use crate::worker::run_job;
    let dir = tempfile::tempdir().unwrap();
    let doc = dir.path().join("code.md");
    std::fs::write(&doc, "# Code\n\n```rust\nfn main() {}\n```\n").unwrap();
    let mut s = AppState::new();
    s.open_paths(std::slice::from_ref(&doc), 0.0);
    for job in s.jobs.drain(..).collect::<Vec<_>>() {
        run_job(job).apply(&mut s);
    }
    let mut rt = rune::Runtime::new(s, view, (1100.0, 760.0), rune::Env { system_dark: false, scale: 2.0 });
    rt.frame(1.0 / 60.0);
    assert!(rt.find("code-copy-0").is_some());
}
