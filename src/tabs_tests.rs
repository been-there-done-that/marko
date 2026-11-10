use super::*;

fn file(dir: &std::path::Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::fs::write(&p, "# t").unwrap();
    p
}

fn paths(set: &TabSet) -> Vec<PathBuf> {
    set.iter().map(|t| t.path.clone()).collect()
}

#[test]
fn open_inserts_after_active_and_activates() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b, c, d) = (file(dir.path(), "a.md"), file(dir.path(), "b.md"), file(dir.path(), "c.md"), file(dir.path(), "d.md"));
    let mut set = TabSet::new();
    let (a_id, _) = set.open(&a);
    let (b_id, _) = set.open(&b);
    set.open(&c);
    set.activate(a_id);
    let (d_id, created) = set.open(&d);
    assert!(created);
    assert_eq!(paths(&set), vec![canonical(&a), canonical(&d), canonical(&b), canonical(&c)]);
    assert_eq!(set.active().unwrap().id, d_id);
    assert_ne!(d_id, b_id);
}

#[test]
fn open_dedupes_by_canonical_path() {
    let dir = tempfile::tempdir().unwrap();
    let a = file(dir.path(), "a.md");
    let other = file(dir.path(), "other.md");
    let mut set = TabSet::new();
    let (first, created) = set.open(&a);
    assert!(created);
    set.open(&other);
    let (dotted, created) = set.open(&dir.path().join("./a.md"));
    assert!(!created);
    assert_eq!(dotted, first);
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&a, dir.path().join("link.md")).unwrap();
        let (linked, created) = set.open(&dir.path().join("link.md"));
        assert!(!created);
        assert_eq!(linked, first);
    }
    assert_eq!(set.iter().count(), 2);
    assert_eq!(set.active().unwrap().id, first);
}

#[test]
fn close_active_picks_right_then_left() {
    let dir = tempfile::tempdir().unwrap();
    let mut set = TabSet::new();
    let (a, _) = set.open(&file(dir.path(), "a.md"));
    let (b, _) = set.open(&file(dir.path(), "b.md"));
    let (c, _) = set.open(&file(dir.path(), "c.md"));
    set.activate(b);
    set.close(b);
    assert_eq!(set.active().unwrap().id, c);
    set.close(c);
    assert_eq!(set.active().unwrap().id, a);
    set.close(a);
    assert!(set.active().is_none());
    assert!(set.is_empty());
}

#[test]
fn reopen_closed_restores_last_closed_path() {
    let dir = tempfile::tempdir().unwrap();
    let (a, b) = (file(dir.path(), "a.md"), file(dir.path(), "b.md"));
    let mut set = TabSet::new();
    set.open(&a);
    let (b_id, _) = set.open(&b);
    set.close(b_id);
    let reopened = set.reopen_closed().unwrap();
    assert_eq!(set.active().unwrap().id, reopened);
    assert_eq!(set.active().unwrap().path, canonical(&b));
}

#[test]
fn activate_index_nine_is_last() {
    let dir = tempfile::tempdir().unwrap();
    let mut set = TabSet::new();
    let ids: Vec<u64> = (0..3).map(|i| set.open(&file(dir.path(), &format!("{i}.md"))).0).collect();
    set.activate_index(0);
    assert_eq!(set.active().unwrap().id, ids[0]);
    set.activate_index(8);
    assert_eq!(set.active().unwrap().id, ids[2]);
}

#[test]
fn cycle_wraps_both_ways() {
    let dir = tempfile::tempdir().unwrap();
    let mut set = TabSet::new();
    let ids: Vec<u64> = (0..3).map(|i| set.open(&file(dir.path(), &format!("{i}.md"))).0).collect();
    assert_eq!(set.active().unwrap().id, ids[2]);
    set.cycle(true);
    assert_eq!(set.active().unwrap().id, ids[0]);
    set.cycle(false);
    assert_eq!(set.active().unwrap().id, ids[2]);
}

#[test]
fn close_others_and_close_to_right() {
    let dir = tempfile::tempdir().unwrap();
    let mut set = TabSet::new();
    let ids: Vec<u64> = (0..4).map(|i| set.open(&file(dir.path(), &format!("{i}.md"))).0).collect();
    set.close_to_right(ids[1]);
    assert_eq!(set.iter().map(|t| t.id).collect::<Vec<_>>(), vec![ids[0], ids[1]]);
    assert_eq!(set.active().unwrap().id, ids[1]);

    let mut set = TabSet::new();
    let ids: Vec<u64> = (0..4).map(|i| set.open(&file(dir.path(), &format!("x{i}.md"))).0).collect();
    set.close_others(ids[1]);
    assert_eq!(set.iter().map(|t| t.id).collect::<Vec<_>>(), vec![ids[1]]);
    assert_eq!(set.active().unwrap().id, ids[1]);
}

#[test]
fn move_tab_reorders_and_keeps_active_id() {
    let dir = tempfile::tempdir().unwrap();
    let mut set = TabSet::new();
    let (a, _) = set.open(&file(dir.path(), "a.md"));
    let (b, _) = set.open(&file(dir.path(), "b.md"));
    let (c, _) = set.open(&file(dir.path(), "c.md"));
    set.activate(b);
    set.move_tab(a, 2);
    assert_eq!(set.iter().map(|t| t.id).collect::<Vec<_>>(), vec![b, c, a]);
    assert_eq!(set.active().unwrap().id, b);
}
