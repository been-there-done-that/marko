use super::*;

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

#[test]
fn a_release_started_from_a_terminal_detaches() {
    assert!(should_detach(&args(&["."]), true, false, true));
    assert!(should_detach(&args(&[]), true, false, true));
    assert!(should_detach(&args(&["notes.md", "docs/"]), true, false, true));
}

#[test]
fn wait_keeps_it_in_the_foreground() {
    assert!(!should_detach(&args(&["--wait", "."]), true, false, true));
    assert!(is_flag("--wait"));
    assert!(!is_flag("notes.md"));
}

#[test]
fn dev_flags_debug_builds_menu_launches_and_the_copy_stay() {
    assert!(!should_detach(&args(&["--snap", "out.png"]), true, false, true), "dev flags print or write");
    assert!(!should_detach(&args(&["--list-scenes"]), true, false, true));
    assert!(!should_detach(&args(&["."]), true, false, false), "debug builds keep their logs");
    assert!(!should_detach(&args(&["."]), false, false, true), "no terminal is waiting");
    assert!(!should_detach(&args(&["."]), true, true, true), "the background copy doesn't detach again");
}
