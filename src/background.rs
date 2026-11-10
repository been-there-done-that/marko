//! The background picture: one image behind the whole window, decoded off the UI thread.
//! `ui::view` draws it under the base view; the panels over it are translucent.

use std::path::{Path, PathBuf};

use rune::Tx;

use crate::app::{AppState, Job};

/// The picture and what it was asked for.
#[derive(Default)]
pub struct BackgroundState {
    /// The decoded picture, once the worker has it.
    pub image: Option<rune::Image>,
    /// The path last queued, so a path change queues once.
    pub for_path: Option<PathBuf>,
    /// The last decode failed; the path is not retried until it changes.
    pub failed: bool,
}

/// Once per frame: when the chosen path differs from the one loaded, drops the old picture
/// and queues a decode. Returns true when the picture was dropped (the screen changes).
pub fn tick(s: &mut AppState, _now: f64, _tx: &mut Tx) -> bool {
    if s.settings.background.path == s.bg.for_path {
        return false;
    }
    let path = s.settings.background.path.clone();
    let had = s.bg.image.take().is_some();
    s.bg.for_path = path.clone();
    s.bg.failed = false;
    if let Some(path) = path {
        s.jobs.push(Job::Background { path });
    }
    had
}

impl AppState {
    /// A worker finished decoding the background. A result for a path that is no longer chosen
    /// is dropped.
    pub fn background_ready(&mut self, path: &Path, image: Option<rune::Image>) {
        if self.bg.for_path.as_deref() != Some(path) {
            return;
        }
        match image {
            Some(img) => self.bg.image = Some(img),
            None => {
                self.bg.failed = true;
                let now = self.now;
                self.show_toast("Couldn't load the background picture", now);
            }
        }
    }

    /// True when a background picture is set, loaded or not: panels are translucent then.
    pub fn has_background(&self) -> bool {
        self.settings.background.path.is_some()
    }
}

#[cfg(test)]
#[path = "background_tests.rs"]
mod tests;
