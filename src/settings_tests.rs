use super::*;

#[test]
fn missing_fields_take_defaults() {
    let s: Settings = serde_json::from_str(r#"{"theme":"paper"}"#).expect("parses");
    assert_eq!(s.theme, "paper");
    assert_eq!(s.text_size, TEXT_SIZE);
    assert_eq!(s.background, Background::default());
    assert_eq!(s.web_images, WebImages::Ask);
}

#[test]
fn clamped_pulls_numbers_into_range() {
    let mut s = Settings::default();
    s.text_size = 400.0;
    s.line_spacing = f32::NAN;
    s.background.panel_opacity = 0.0;
    let s = s.clamped();
    assert_eq!(s.text_size, TEXT_SIZE_MAX);
    assert_eq!(s.line_spacing, LINE_SPACING);
    assert_eq!(s.background.panel_opacity, PANEL_MIN);
}

#[test]
fn panels_are_solid_without_a_picture() {
    let mut s = Settings::default();
    s.background.panel_opacity = 0.5;
    assert_eq!(s.panel_alpha(), 1.0);
    s.background.path = Some("/x.png".into());
    assert_eq!(s.panel_alpha(), 0.5);
}

#[test]
fn intern_returns_one_pointer_per_value() {
    let a = intern("Fira Code");
    let b = intern(&String::from("Fira Code"));
    assert!(std::ptr::eq(a, b));
}
