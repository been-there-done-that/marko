//! The worker thread: runs `Job`s off the UI thread and posts each result back to the app.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};

use rune::AppHandle;

use crate::app::{AppState, Job};
use crate::edit::save_atomic;
use crate::images::{self, Resolved};
use crate::load::{self, DocState};
use crate::sidebar;

/// The background picture is decoded down to this width (4K).
const BACKGROUND_DECODE_WIDTH: u32 = 3840;

/// What a finished job hands back to the UI thread.
pub enum JobResult {
    /// `images` are the document's local images, resolved to absolute paths.
    Loaded { tab: u64, doc: DocState, images: Vec<PathBuf> },
    Scanned { root: PathBuf, files: Result<Vec<PathBuf>, String> },
    /// A save finished. `text` is what the write was given.
    Saved { tab: u64, text: String, result: Result<(), String> },
    /// `image` is `None` when the file could not be decoded.
    Image { key: PathBuf, image: Option<(rune::Image, usize)> },
    /// The background picture, `None` when it could not be decoded.
    Background { path: PathBuf, image: Option<rune::Image> },
}

/// Runs one job to completion. Blocking; call only from the worker.
pub fn run_job(job: Job) -> JobResult {
    match job {
        Job::Load { tab, path } => {
            let doc = load::load(&path);
            let images = local_images(&path, &doc);
            JobResult::Loaded { tab, doc, images }
        }
        Job::Image { key } => {
            let image = images::decode(&key, images::MAX_DECODE_WIDTH);
            JobResult::Image { key, image }
        }
        Job::Background { path } => {
            let image = images::decode(&path, BACKGROUND_DECODE_WIDTH).map(|(img, _)| img);
            JobResult::Background { path, image }
        }
        Job::Scan { root } => {
            let files = sidebar::scan(&root);
            JobResult::Scanned { root, files }
        }
        Job::Save { tab, path, text } => {
            let result = save_atomic(&path, &text);
            JobResult::Saved { tab, text, result }
        }
    }
}

impl JobResult {
    /// Applies a result on the UI thread.
    pub fn apply(self, s: &mut AppState) {
        match self {
            JobResult::Loaded { tab, doc, images } => {
                s.loaded(tab, doc);
                s.want_images(images);
            }
            JobResult::Scanned { root, files } => s.scanned(&root, files),
            JobResult::Saved { tab, text, result } => s.saved(tab, text, result),
            JobResult::Image { key, image } => s.image_ready(key, image),
            JobResult::Background { path, image } => s.background_ready(&path, image),
        }
    }
}

/// The local images a loaded document shows, resolved against its folder. Frontmatter is
/// skipped, as the view skips it. Runs on the worker: parsing the source is not UI work.
fn local_images(path: &Path, doc: &DocState) -> Vec<PathBuf> {
    let DocState::Ready { src, .. } = doc else { return Vec::new() };
    let mut out: Vec<PathBuf> = images::image_srcs(load::strip_frontmatter(src))
        .iter()
        .filter_map(|src| match images::resolve(path, src) {
            Resolved::Local(p) => Some(p),
            _ => None,
        })
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Starts the worker thread. Jobs run in the order they are sent; each result is posted
/// to the UI thread with `AppHandle::update`.
pub fn spawn_worker(handle: AppHandle<AppState>) -> Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    std::thread::Builder::new()
        .name("worker".into())
        .spawn(move || {
            for job in rx {
                let r = run_job(job);
                handle.update(move |s, _| r.apply(s));
            }
        })
        .expect("the OS refused to start the worker thread");
    tx
}

#[cfg(test)]
#[path = "worker_tests.rs"]
mod tests;
