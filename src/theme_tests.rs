use super::*;
use crate::icons;

fn hex(s: &str) -> rune::Color {
    rune::color::hex(s).expect("valid test colour")
}

#[test]
fn icon_names_unique_and_bodies_use_current_color() {
    let mut seen = std::collections::HashSet::new();
    for icon in icons::ALL {
        assert!(seen.insert(icon.name), "duplicate icon name {}", icon.name);
        assert!(icon.name.starts_with("lucide-"), "{} lacks the lucide- prefix", icon.name);
        assert_eq!(icon.view, 24.0, "{} view box", icon.name);
        assert!(icon.body.contains("stroke=\"currentColor\""), "{} strokes must use currentColor", icon.name);
        assert!(!icon.body.contains("stroke=\"#"), "{} has a hard-coded stroke colour", icon.name);
    }
    assert_eq!(seen.len(), icons::ALL.len());
}

#[test]
fn tokens_match_spec_samples() {
    let light = tokens(false);
    let dark = tokens(true);
    assert_eq!(light.accent, hex("#2F6BFF"));
    assert_eq!(dark.bar, hex("#1F1F23"));
}

#[test]
fn kit_is_cached() {
    assert!(std::ptr::eq(kit(false), kit(false)));
    assert!(std::ptr::eq(kit(true), kit(true)));
    assert!(!std::ptr::eq(kit(false), kit(true)));
}
