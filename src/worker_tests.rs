use super::*;
use std::path::Path;
use crate::app::AppState;
use crate::load::DocState;

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures").join(name)
}

#[test]
fn worker_runs_load_and_posts_result() {
    let mut s = AppState::new();
    s.open_paths(&[fixture("hello.md")], 0.0);
    let jobs: Vec<Job> = s.jobs.drain(..).collect();
    assert_eq!(jobs.len(), 1);
    let tab = match &jobs[0] {
        Job::Load { tab, .. } => *tab,
        other => panic!("expected a Load job, got {other:?}"),
    };
    assert!(matches!(s.tabs.get_mut(tab).unwrap().doc, DocState::Loading));

    let r = run_job(jobs.into_iter().next().unwrap());
    assert!(matches!(&r, JobResult::Loaded { doc: DocState::Ready { .. }, .. }));
    r.apply(&mut s);

    assert!(matches!(s.tabs.get_mut(tab).unwrap().doc, DocState::Ready { .. }));
}

#[test]
fn run_job_scans_folder() {
    let dir = fixture("");
    match run_job(Job::Scan { root: dir.clone() }) {
        JobResult::Scanned { root, files: Ok(files) } => {
            assert_eq!(root, dir);
            assert!(files.iter().any(|f| f.ends_with("basic.md")));
            assert!(files.iter().any(|f| f.ends_with("hello.md")));
        }
        _ => panic!("expected a successful scan"),
    }
}
