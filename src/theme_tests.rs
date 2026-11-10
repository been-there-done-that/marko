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

fn with_theme(id: &str) -> Settings {
    Settings { theme: id.into(), ..Settings::default() }
}

#[test]
fn auto_follows_the_system() {
    let light = look(&Settings::default(), &[], false);
    let dark = look(&Settings::default(), &[], true);
    assert_eq!(light.tokens.accent, hex("#2F6BFF"));
    assert_eq!(dark.tokens.bar, hex("#1F1F23"));
    assert!(!light.dark && dark.dark);
}

#[test]
fn looks_are_cached_per_choice() {
    let s = with_theme("paper");
    assert!(std::ptr::eq(look(&s, &[], false), look(&s, &[], true)), "a fixed theme ignores the system");
    assert!(!std::ptr::eq(look(&s, &[], false), look(&with_theme("nordic"), &[], false)));
}

#[test]
fn unknown_theme_falls_back_to_light_or_dark() {
    assert_eq!(look(&with_theme("gone"), &[], true).tokens.bar, hex("#1F1F23"));
}

#[test]
fn accent_override_and_translucent_panels() {
    let mut s = with_theme("light");
    s.accent = Some("#E5484D".into());
    s.background.path = Some("/pic.png".into());
    s.background.panel_opacity = 0.5;
    let l = look(&s, &[], false);
    assert_eq!(l.tokens.accent, hex("#E5484D"));
    assert!((l.tokens.side.a - 0.5).abs() < 0.01);
    assert!((l.kit.c.page.a - 0.5).abs() < 0.01);
    assert_eq!(l.tokens.code_bg.a, 1.0);
}

#[test]
fn user_theme_is_found_by_id() {
    let mut mine = crate::themes::builtin().remove(5);
    mine.id = "user:mine".into();
    mine.accent = "#123456".into();
    assert_eq!(look(&with_theme("user:mine"), &[mine], false).tokens.accent, hex("#123456"));
}

#[test]
fn text_size_and_code_font_reach_the_style() {
    let mut s = Settings::default();
    s.code_font = "Fira Code".into();
    s.ligatures = true;
    let k = &look(&s, &[], false).kit;
    let code = code_font(k, &s);
    assert_eq!(code.family, rune::Family::Named("Fira Code"));
    assert!(code.features.contains("'liga' 1"));
    s.ligatures = false;
    assert!(code_font(k, &s).features.contains("'liga' 0"));
    s.code_font.clear();
    assert_eq!(code_font(k, &s).family, k.ty.mono.family);
}
