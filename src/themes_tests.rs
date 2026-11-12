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

#[test]
fn valid_hex_rejects_multibyte_without_panicking() {
    assert!(valid_hex("#fff"));
    assert!(valid_hex("#ffffff"));
    assert!(valid_hex("#ffffffaa"));
    assert!(!valid_hex("#éa"), "a 3-byte non-ASCII value is refused, not sliced");
    assert!(!valid_hex("ffffff"));
    assert!(!valid_hex("#ggg"));
}

#[test]
fn a_multibyte_theme_colour_is_invalid_not_a_panic() {
    let mut def = builtin().remove(0);
    def.bg = "#éa".into();
    assert!(!valid(&def));
}

#[test]
fn oversized_theme_file_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("big.json"), "x".repeat(MAX_THEME_BYTES as usize + 1)).unwrap();
    let (themes, errors) = load_dir(dir.path());
    assert!(themes.is_empty());
    assert_eq!(errors, vec!["big.json".to_string()]);
}
