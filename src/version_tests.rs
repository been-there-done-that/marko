use super::*;

#[test]
fn the_label_names_the_version_and_the_commit() {
    let label = label();
    assert!(label.starts_with(&format!("Marko {} (", env!("CARGO_PKG_VERSION"))), "{label}");
    assert!(label.ends_with(')'), "{label}");
}
