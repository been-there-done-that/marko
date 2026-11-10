//! The installed font families the Settings panel offers. Filled after launch by the font
//! loader (`fonts::load`, off the UI thread); empty until then.

/// Installed families, sorted, without hidden (`.`-prefixed) ones.
#[derive(Default, Debug, Clone, PartialEq)]
pub struct FontList {
    /// Every installed family.
    pub all: Vec<String>,
    /// The monospaced ones, for the code font.
    pub mono: Vec<String>,
    /// True once the loader has answered (the lists may still be empty).
    pub loaded: bool,
}
