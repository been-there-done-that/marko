use super::*;

fn touch(dir: &Path, rel: &str) {
    let p = dir.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, "# x").unwrap();
}

#[test]
fn scan_finds_markdown_sorted() {
    let dir = tempfile::tempdir().unwrap();
    for f in ["z.md", "b/a.markdown", "a.md", "notes.png", "b/c.txt"] {
        touch(dir.path(), f);
    }
    let found = scan(dir.path()).unwrap();
    assert_eq!(found, vec![PathBuf::from("a.md"), PathBuf::from("b/a.markdown"), PathBuf::from("b/c.txt"), PathBuf::from("z.md")]);
}

#[test]
fn scan_skips_ignored_and_caps() {
    let dir = tempfile::tempdir().unwrap();
    for f in ["node_modules/x.md", ".hidden/y.md", "target/z.md", ".venv/v.md", "out/w.md", "keep.md"] {
        touch(dir.path(), f);
    }
    std::fs::write(dir.path().join(".gitignore"), "out/\n").unwrap();
    let found = scan(dir.path()).unwrap();
    assert_eq!(found, vec![PathBuf::from("keep.md")]);
}

#[test]
fn scan_with_cap_stops_at_cap() {
    let dir = tempfile::tempdir().unwrap();
    for i in 0..5 {
        touch(dir.path(), &format!("f{i}.md"));
    }
    assert_eq!(scan_with_cap(dir.path(), 3).unwrap().len(), 3);
}

#[test]
fn scan_missing_folder_is_error() {
    let dir = tempfile::tempdir().unwrap();
    assert!(scan(&dir.path().join("nope")).is_err());
}

#[test]
fn add_root_detects_duplicates_and_nesting() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("sub")).unwrap();
    let mut s = Sidebar::new();
    assert!(matches!(s.add_root(dir.path()), AddRoot::Added(0)));
    assert!(s.roots[0].scanning);
    assert!(matches!(s.add_root(&dir.path().join("./")), AddRoot::AlreadyRoot(0)));
    assert!(matches!(s.add_root(&dir.path().join("sub")), AddRoot::InsideRoot { root: 0 }));
    assert_eq!(s.roots.len(), 1);
    s.remove_root(0);
    assert!(s.roots.is_empty());
}

#[test]
fn root_of_matches_canonical_paths() {
    let dir = tempfile::tempdir().unwrap();
    touch(dir.path(), "a.md");
    let other = tempfile::tempdir().unwrap();
    let mut s = Sidebar::new();
    s.add_root(dir.path());
    s.add_root(other.path());
    assert_eq!(s.root_of(&canonical(&dir.path().join("a.md"))), Some(0));
    assert_eq!(s.root_of(&canonical(&other.path().join("./b.md"))), Some(1));
    assert_eq!(s.root_of(Path::new("/definitely/not/here.md")), None);
}

#[test]
fn readme_or_first_prefers_readme_any_case() {
    let v = |xs: &[&str]| xs.iter().map(PathBuf::from).collect::<Vec<_>>();
    assert_eq!(readme_or_first(&v(&["a.md", "sub/readme.md", "readme.MD"])), Some(PathBuf::from("readme.MD")));
    assert_eq!(readme_or_first(&v(&["a.md", "b.md"])), Some(PathBuf::from("a.md")));
    assert_eq!(readme_or_first(&[]), None);
}

#[test]
fn build_tree_nests_folders_dirs_first() {
    let files = vec![PathBuf::from("a.md"), PathBuf::from("sub/x.md"), PathBuf::from("sub/deep/y.md")];
    let tree = build_tree(&files, Path::new("/base"));
    let top: Vec<&str> = tree.children(None).collect();
    assert_eq!(top.len(), 2, "{top:?}");
    assert!(tree.get(top[0]).is_some_and(|n| n.dir), "folder first: {top:?}");
    assert!(tree.get(top[1]).is_some_and(|n| !n.dir), "file second: {top:?}");
    let sub = top[0];
    assert!(tree.children(Some(sub)).count() >= 1);
}
