use super::*;
use std::path::PathBuf;

const MB: u64 = 1024 * 1024;

fn write(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
    let p = dir.path().join(name);
    std::fs::write(&p, bytes).unwrap();
    p
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

#[test]
fn load_reads_utf8() {
    match load(&fixture("basic.md")) {
        DocState::Ready { src, large, lossy } => {
            assert!(src.contains("# Basic document"));
            assert!(src.contains("```mermaid"));
            assert!(!large);
            assert!(!lossy);
        }
        _ => panic!("expected Ready"),
    }
}

#[test]
fn load_flags_lossy_for_invalid_utf8() {
    let dir = tempfile::tempdir().unwrap();
    let p = write(&dir, "bad.md", b"ok \xff\xfe");
    match load(&p) {
        DocState::Ready { src, lossy, .. } => {
            assert!(lossy);
            assert!(src.contains("ok"));
        }
        _ => panic!("expected Ready"),
    }
}

#[test]
fn load_refuses_binary() {
    let dir = tempfile::tempdir().unwrap();
    let mut bytes = vec![b'a'; 64];
    bytes[10] = 0;
    let p = write(&dir, "bin.md", &bytes);
    assert!(matches!(load(&p), DocState::Error(LoadError::Binary)));
}

#[test]
fn load_flags_large_over_5mb() {
    let dir = tempfile::tempdir().unwrap();
    let p = write(&dir, "big.md", &vec![b'a'; (5 * MB + 1) as usize]);
    match load(&p) {
        DocState::Ready { large, .. } => assert!(large),
        _ => panic!("expected Ready"),
    }
}

#[test]
fn load_refuses_over_50mb() {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join("huge.md");
    // Sparse file: no content is written, so a read would return zeros (and would be
    // refused as binary). TooLarge proves the size check ran before any read.
    let f = std::fs::File::create(&p).unwrap();
    f.set_len(50 * MB + 1).unwrap();
    assert!(matches!(load(&p), DocState::Error(LoadError::TooLarge { bytes }) if bytes == 50 * MB + 1));
}

#[test]
fn load_missing_is_not_found() {
    let dir = tempfile::tempdir().unwrap();
    assert!(matches!(load(&dir.path().join("nope.md")), DocState::Error(LoadError::NotFound)));
}

#[test]
fn strip_frontmatter_yaml_and_toml() {
    assert_eq!(strip_frontmatter("---\na: 1\n---\n# T"), "# T");
    assert_eq!(strip_frontmatter("+++\na=1\n+++\n# T"), "# T");
    assert_eq!(strip_frontmatter("# T\n---\n"), "# T\n---\n");
    assert_eq!(strip_frontmatter("---\nunterminated"), "---\nunterminated");
}

#[test]
fn strip_frontmatter_reads_fixture() {
    let src = std::fs::read_to_string(fixture("frontmatter.md")).unwrap();
    assert!(strip_frontmatter(&src).starts_with("# Frontmatter example"));
}
