use super::*;

fn args(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| s.to_string()).collect()
}

#[test]
fn version_flags() {
    assert_eq!(command(&args(&["--version"])), Some(Command::Version));
    assert_eq!(command(&args(&["-V"])), Some(Command::Version));
    assert_eq!(command(&args(&["--version", "notes.md"])), Some(Command::Version));
}

#[test]
fn update_and_upgrade() {
    assert_eq!(command(&args(&["update"])), Some(Command::Update { check: false }));
    assert_eq!(command(&args(&["upgrade"])), Some(Command::Update { check: false }));
    assert_eq!(command(&args(&["update", "--check"])), Some(Command::Update { check: true }));
    assert_eq!(command(&args(&["update", "-n"])), Some(Command::Update { check: true }));
}

#[test]
fn help() {
    assert_eq!(command(&args(&["--help"])), Some(Command::Help));
    assert_eq!(command(&args(&["-h"])), Some(Command::Help));
    assert_eq!(command(&args(&["update", "--help"])), Some(Command::Help));
}

#[test]
fn files_and_wait_are_not_commands() {
    assert_eq!(command(&args(&["notes.md"])), None);
    assert_eq!(command(&args(&["--wait", "notes.md"])), None);
    assert_eq!(command(&args(&[])), None);
}
