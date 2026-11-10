use super::*;

#[test]
fn is_markdown_accepts_listed_extensions_any_case() {
    for p in ["a.md", "B.MARKDOWN", "c.Mdown", "d.mkd", "e.txt"] { assert!(is_markdown(Path::new(p)), "{p}"); }
    for p in ["f.png", "g", ".md"] { assert!(!is_markdown(Path::new(p)), "{p}"); }
}

#[cfg(unix)]
#[test]
fn canonical_resolves_dot_segments_and_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.md"), "# a").unwrap();
    std::os::unix::fs::symlink(dir.path().join("a.md"), dir.path().join("link.md")).unwrap();
    assert_eq!(canonical(&dir.path().join("./a.md")), canonical(&dir.path().join("link.md")));
}

#[test]
fn display_titles_disambiguates_duplicates() {
    let v: Vec<PathBuf> = ["/x/tables/README.md", "/x/rune/README.md", "/x/rune/PLAN.md"].iter().map(PathBuf::from).collect();
    assert_eq!(display_titles(&v), ["README.md — tables", "README.md — rune", "PLAN.md"]);
}
