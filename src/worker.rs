//! The worker thread: runs `Job`s off the UI thread and posts each result back to the app.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};

use rune::AppHandle;
use rune_kit::{FileNode, TreeModel};

use crate::app::{AppState, Job};
use crate::edit::save_atomic;
use crate::images;
use crate::load::{self, DocState};
use crate::sidebar;

/// The background picture is decoded down to this width (4K).
/// Wide enough for a 4K background without holding a full-resolution photo in memory. The
/// background is blurred behind a scrim, so 2560 px does not look soft on a high-DPI screen.
const BACKGROUND_DECODE_WIDTH: u32 = 2560;

/// What a finished job hands back to the UI thread.
pub enum JobResult {
    /// `images` are the document's local images, resolved to absolute paths; `web` its web image addresses.
    Loaded { tab: u64, doc: DocState, images: Vec<PathBuf>, web: Vec<String> },
    Scanned { root: PathBuf, tree: Box<Result<TreeModel<FileNode<PathBuf>>, String>>, open: Option<PathBuf> },
    /// A save finished. `text` is what the write was given.
    Saved { tab: u64, text: String, result: Result<(), String> },
    /// `image` is `None` when the file could not be decoded.
    Image { key: PathBuf, image: Option<(rune::Image, usize)> },
    /// The background picture, `None` when it could not be decoded.
    Background { path: PathBuf, image: Option<rune::Image> },
}

/// What the `Loaded` result's image lists hold.
struct DocImages {
    local: Vec<PathBuf>,
    web: Vec<String>,
}

/// Runs one job to completion. Blocking; call only from the worker.
pub fn run_job(job: Job) -> JobResult {
    match job {
        Job::Load { tab, path } => {
            let doc = load::load(&path);
            let DocImages { local, web } = doc_images(&path, &doc);
            JobResult::Loaded { tab, doc, images: local, web }
        }
        Job::Image { key } => {
            let image = images::decode(&key, images::MAX_DECODE_WIDTH);
            JobResult::Image { key, image }
        }
        Job::Background { path } => {
            let image = images::decode(&path, BACKGROUND_DECODE_WIDTH).map(|(img, _)| img);
            JobResult::Background { path, image }
        }
        Job::WebImage { url } => {
            // A failed fetch or decode is `None`: the document keeps the image's alt text.
            let image = crate::net::fetch(&url).ok().and_then(|bytes| images::decode_bytes(&bytes, images::MAX_DECODE_WIDTH));
            JobResult::Image { key: images::web_key(&url), image }
        }
        Job::Scan { root } => match sidebar::scan(&root) {
            Ok(files) => {
                let open = sidebar::readme_or_first(&files);
                let tree = sidebar::build_tree(&files, &root);
                JobResult::Scanned { root, tree: Box::new(Ok(tree)), open }
            }
            Err(e) => JobResult::Scanned { root, tree: Box::new(Err(e)), open: None },
        },
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
            JobResult::Loaded { tab, doc, images, web } => {
                s.loaded(tab, doc);
                s.want_images(images);
                s.doc_web_images(tab, web);
            }
            JobResult::Scanned { root, tree, open } => s.scanned(&root, *tree, open),
            JobResult::Saved { tab, text, result } => s.saved(tab, text, result),
            JobResult::Image { key, image } => s.image_ready(key, image),
            JobResult::Background { path, image } => s.background_ready(&path, image),
        }
    }
}

/// The images a loaded document shows: local ones resolved against its folder, and web ones as
/// written. Frontmatter is skipped, as the view skips it. Runs on the worker: parsing the
/// source is not UI work.
fn doc_images(path: &Path, doc: &DocState) -> DocImages {
    let DocState::Ready { src, .. } = doc else { return DocImages { local: Vec::new(), web: Vec::new() } };
    let (mut local, mut web) = images::split_srcs(path, &images::image_srcs(load::strip_frontmatter(src)));
    local.sort();
    local.dedup();
    web.sort();
    web.dedup();
    DocImages { local, web }
}

/// Starts the worker thread. Jobs run in the order they are sent; each result is posted
/// to the UI thread with `AppHandle::update`.
pub fn spawn_worker(handle: AppHandle<AppState>) -> Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    // Network fetches run on their own thread so a slow server cannot hold up a save or a load.
    let (web_tx, web_rx) = mpsc::channel::<Job>();
    let web_handle = handle.clone();
    std::thread::Builder::new()
        .name("web-worker".into())
        .spawn(move || {
            for job in web_rx {
                let r = run_job(job);
                web_handle.update(move |s, _| r.apply(s));
            }
        })
        .expect("the OS refused to start the web worker thread");
    std::thread::Builder::new()
        .name("worker".into())
        .spawn(move || {
            for job in rx {
                if matches!(job, Job::WebImage { .. }) {
                    if web_tx.send(job).is_err() {
                        break;
                    }
                    continue;
                }
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
