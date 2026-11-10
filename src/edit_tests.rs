use super::*;

#[test]
fn dirty_after_edit_clean_after_save() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.md");
    std::fs::write(&path, "# a\n").unwrap();
    let mut buf = EditBuffer::from_src("# a\n");
    assert!(!buf.dirty());
    buf.text.replace_text("# a\n\nmore\n");
    assert!(buf.dirty());
    let text = buf.text();
    save_atomic(&path, &text).unwrap();
    buf.mark_saved(&text);
    assert!(!buf.dirty());
}

#[test]
fn save_atomic_replaces_contents() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.md");
    std::fs::write(&path, "old").unwrap();
    save_atomic(&path, "new\n").unwrap();
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
    assert!(!dir.path().join("a.md.tmp").exists());
}

#[test]
fn save_atomic_into_missing_dir_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing").join("a.md");
    assert!(save_atomic(&path, "text").is_err());
}

#[test]
fn untitled_names_increment() {
    assert_eq!(untitled_name(&[]), "Untitled.md");
    assert_eq!(untitled_name(&["Untitled.md".to_string()]), "Untitled 2.md");
    let two = ["Untitled.md".to_string(), "Untitled 2.md".to_string()];
    assert_eq!(untitled_name(&two), "Untitled 3.md");
    assert_eq!(untitled_name(&["Untitled 2.md".to_string()]), "Untitled.md");
}

#[test]
fn own_save_is_not_external_change() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.md");
    std::fs::write(&path, "x").unwrap();
    let mut buf = EditBuffer::from_src("x");
    buf.text.replace_text("y\n");
    let text = buf.text();
    save_atomic(&path, &text).unwrap();
    buf.mark_saved(&text);
    let on_disk = std::fs::read_to_string(&path).unwrap();
    assert_eq!(content_hash(&on_disk), buf.saved_hash);
}

#[test]
fn typing_during_save_stays_dirty() {
    let mut buf = EditBuffer::from_src("a");
    buf.text.replace_text("ab");
    let saved = buf.text();
    buf.text.replace_text("abc");
    buf.mark_saved(&saved);
    assert!(buf.dirty());
}
