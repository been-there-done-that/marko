use sha2::{Digest, Sha256};

use super::*;

#[test]
fn newer_versions() {
    assert!(is_newer("0.2.0", "0.1.2"));
    assert!(is_newer("v1.0.0", "0.9.9"));
    assert!(!is_newer("0.1.2", "0.1.2"));
    assert!(!is_newer("0.1.1", "0.1.2"));
    assert!(!is_newer("v0.1.2", "0.1.2"));
}

#[test]
fn version_parts_ignore_a_leading_v() {
    assert_eq!(version_parts("v0.1.2"), vec![0, 1, 2]);
    assert_eq!(version_parts("1.2.3-beta.4"), vec![1, 2, 3, 4]);
}

#[test]
fn release_assets_match_the_workflow() {
    assert_eq!(asset_name("linux", "x86_64"), Some("marko-linux-x86_64.tar.gz"));
    assert_eq!(asset_name("linux", "aarch64"), Some("marko-linux-aarch64.tar.gz"));
    assert_eq!(asset_name("macos", "aarch64"), Some("marko-macos-aarch64.tar.gz"));
    assert_eq!(asset_name("macos", "x86_64"), Some("marko-macos-x86_64.tar.gz"));
    assert_eq!(asset_name("windows", "x86_64"), Some("marko-windows-x86_64.zip"));
    assert_eq!(asset_name("freebsd", "x86_64"), None);
}

#[test]
fn sums_parse_gnu_and_bsd_lines() {
    let text = "abc123  marko-linux-x86_64.tar.gz\nDEF456 *marko-windows-x86_64.zip\n";
    assert_eq!(parse_sums(text, "marko-linux-x86_64.tar.gz"), Some("abc123".into()));
    assert_eq!(parse_sums(text, "marko-windows-x86_64.zip"), Some("def456".into()));
    assert_eq!(parse_sums(text, "missing.tar.gz"), None);
}

#[test]
fn sha256_of_abc() {
    assert_eq!(hex(&Sha256::digest(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}
