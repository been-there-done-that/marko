//! What the reader has chosen to make the app their own: theme, accent, fonts, text size,
//! background picture, and how web images load. Saved with the session (`Session::settings`)
//! and read before the first frame, so a launch never flashes the defaults.

use std::path::PathBuf;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

/// Default body size in points, before zoom (§4.7).
pub const TEXT_SIZE: f32 = 16.0;
pub const TEXT_SIZE_MIN: f32 = 12.0;
pub const TEXT_SIZE_MAX: f32 = 24.0;
/// Default line height as a multiple of the body size.
pub const LINE_SPACING: f32 = 1.7;
pub const LINE_SPACING_MIN: f32 = 1.2;
pub const LINE_SPACING_MAX: f32 = 2.2;
/// The theme id that follows the system between `light` and `dark`.
pub const AUTO_THEME: &str = "auto";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// A theme id from `themes::all`, or `AUTO_THEME`.
    pub theme: String,
    /// Replaces the theme's accent, as `#rrggbb`. None keeps the theme's own.
    pub accent: Option<String>,
    /// Family for document text (an installed family name). Empty means the default (Inter).
    pub body_font: String,
    /// Family for code blocks and the editor. Empty means the system monospace.
    pub code_font: String,
    /// Joins `=>`, `!=` and the like in code fonts that have ligatures.
    pub ligatures: bool,
    /// Body size in points before zoom.
    pub text_size: f32,
    /// Body line height as a multiple of the size.
    pub line_spacing: f32,
    pub background: Background,
    pub web_images: WebImages,
    /// Shows the reading-progress line and percentage pill.
    pub show_progress: bool,
    pub startup: Startup,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: AUTO_THEME.into(),
            accent: None,
            body_font: String::new(),
            code_font: String::new(),
            ligatures: false,
            text_size: TEXT_SIZE,
            line_spacing: LINE_SPACING,
            background: Background::default(),
            web_images: WebImages::Ask,
            show_progress: true,
            startup: Startup::Restore,
        }
    }
}

impl Settings {
    /// Pulls every number back into its range, so a hand-edited file can't break layout.
    pub fn clamped(mut self) -> Self {
        let fix = |v: f32, lo: f32, hi: f32, def: f32| if v.is_finite() { v.clamp(lo, hi) } else { def };
        self.text_size = fix(self.text_size, TEXT_SIZE_MIN, TEXT_SIZE_MAX, TEXT_SIZE);
        self.line_spacing = fix(self.line_spacing, LINE_SPACING_MIN, LINE_SPACING_MAX, LINE_SPACING);
        let b = &mut self.background;
        b.image_opacity = fix(b.image_opacity, 0.0, 1.0, 1.0);
        b.panel_opacity = fix(b.panel_opacity, PANEL_MIN, 1.0, 0.82);
        b.blur = fix(b.blur, 0.0, BLUR_MAX, 12.0);
        self
    }

    /// The panel opacity the theme should draw with: 1.0 without a background picture.
    pub fn panel_alpha(&self) -> f32 {
        if self.background.path.is_some() { self.background.panel_opacity } else { 1.0 }
    }
}

/// Lowest panel opacity: below this, text over a busy picture stops being readable.
pub const PANEL_MIN: f32 = 0.35;
/// Most backdrop blur, in points.
pub const BLUR_MAX: f32 = 40.0;

/// One picture behind the whole window.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Background {
    /// The picture file. None draws the theme's plain background.
    pub path: Option<PathBuf>,
    /// How strongly the picture shows, 0 to 1.
    pub image_opacity: f32,
    /// How solid the sidebar, titlebar and reading surface are over it, `PANEL_MIN` to 1.
    pub panel_opacity: f32,
    /// Frosted-glass blur of the picture behind the panels, in points (0 is off).
    pub blur: f32,
    pub fit: Fit,
}

impl Default for Background {
    fn default() -> Self {
        Self { path: None, image_opacity: 1.0, panel_opacity: 0.82, blur: 12.0, fit: Fit::Cover }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Fit {
    /// Fills the window, cropping the picture.
    Cover,
    /// Shows the whole picture, centred.
    Contain,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WebImages {
    /// A web image shows a placeholder until it is clicked (loading it tells its server you opened the file).
    Ask,
    /// Web images load as the document opens.
    Always,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Startup {
    /// Reopens the last session's tabs and folders.
    Restore,
    /// Starts with no tabs.
    Empty,
}

/// Settings read from disk before the window opens. `AppState::new` starts from these.
static INITIAL: OnceLock<Settings> = OnceLock::new();

/// Records the saved settings for `AppState::new`. Call once, before `App::run`.
pub fn set_initial(s: Settings) {
    let _ = INITIAL.set(s.clamped());
}

/// The settings a new `AppState` starts with: the saved ones, or the defaults (tests, first launch).
pub fn initial() -> Settings {
    INITIAL.get().cloned().unwrap_or_default()
}

/// A `&'static str` for a runtime string. Rune's `Family::Named` and `TextStyle::features` take
/// `&'static str`; each distinct value leaks once, and there are only as many as the reader picks.
pub fn intern(s: &str) -> &'static str {
    // A hand-edited settings file could name an enormous font; do not leak it.
    if s.len() > 256 {
        return "";
    }
    use std::collections::HashSet;
    use std::sync::Mutex;
    static SEEN: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    let mut seen = SEEN.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    if let Some(hit) = seen.get(s) {
        return hit;
    }
    let leaked: &'static str = Box::leak(s.to_owned().into_boxed_str());
    seen.insert(leaked);
    leaked
}

#[cfg(test)]
#[path = "settings_tests.rs"]
mod tests;
