use super::*;

#[test]
fn parse_dedupes_sorts_and_trims() {
    let out = "Zed Sans\nalpha\n.Hidden Font\n\nAlpha\n  Noto Sans, Noto Sans Display \nDejaVu Sans Mono\nzed sans\n";
    assert_eq!(parse_fc_list(out), vec!["alpha", "DejaVu Sans Mono", "Noto Sans", "Zed Sans"]);
}

#[test]
fn parse_empty_output() {
    assert!(parse_fc_list("").is_empty());
    assert!(parse_fc_list("\n\n .x\n").is_empty());
}

#[test]
fn non_linux_or_failed_load_is_loaded_and_may_be_empty() {
    let list = load();
    assert!(list.loaded);
    assert!(list.mono.iter().all(|m| list.all.contains(m)) || list.all.is_empty());
}
