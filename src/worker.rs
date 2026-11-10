//! The worker thread: runs `Job`s off the UI thread and posts each result back to the app.

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};

use rune::AppHandle;

use crate::app::{AppState, Job};
use crate::edit::save_atomic;
use crate::load::{self, DocState};
use crate::sidebar;

/// What a finished job hands back to the UI thread.
pub enum JobResult {
    Loaded { tab: u64, doc: DocState },
    Scanned { root: PathBuf, files: Result<Vec<PathBuf>, String> },
    /// A save finished. `text` is what the write was given.
    Saved { tab: u64, text: String, result: Result<(), String> },
}

/// Runs one job to completion. Blocking; call only from the worker.
pub fn run_job(job: Job) -> JobResult {
    match job {
        Job::Load { tab, path } => JobResult::Loaded { tab, doc: load::load(&path) },
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
            JobResult::Loaded { tab, doc } => s.loaded(tab, doc),
            JobResult::Scanned { root, files } => s.scanned(&root, files),
            JobResult::Saved { tab, text, result } => s.saved(tab, text, result),
        }
    }
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
