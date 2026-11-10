use super::*;

#[test]
fn builtins_are_valid_and_unique() {
    let all = builtin();
    let mut ids: Vec<&str> = all.iter().map(|t| t.id.as_str()).collect();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), all.len());
    assert!(all.iter().all(valid), "a built-in theme has a bad colour");
    assert!(find("light", &all, &[]).is_some_and(|t| !t.dark));
    assert!(find("dark", &all, &[]).is_some_and(|t| t.dark));
}

#[test]
fn accents_parse() {
    assert!(ACCENTS.iter().all(|(_, c)| rune::color::hex(c).is_some()));
}

#[test]
fn load_dir_reads_good_files_and_names_bad_ones() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut good = builtin().remove(2);
    good.name = "My Paper".into();
    std::fs::write(dir.path().join("mine.json"), serde_json::to_string(&good).expect("json")).expect("write");
    let mut bad = good.clone();
    bad.bg = "red".into();
    std::fs::write(dir.path().join("bad.json"), serde_json::to_string(&bad).expect("json")).expect("write");
    std::fs::write(dir.path().join("junk.json"), "{").expect("write");
    std::fs::write(dir.path().join("notes.txt"), "x").expect("write");
    let (themes, mut errors) = load_dir(dir.path());
    errors.sort();
    assert_eq!(themes.len(), 1);
    assert_eq!(themes[0].id, "user:mine");
    assert_eq!(themes[0].name, "My Paper");
    assert_eq!(errors, vec!["bad.json".to_string(), "junk.json".to_string()]);
}

#[test]
fn missing_dir_is_empty() {
    let (themes, errors) = load_dir(Path::new("/nonexistent/marko-themes"));
    assert!(themes.is_empty() && errors.is_empty());
}
