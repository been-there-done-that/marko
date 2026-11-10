use std::path::PathBuf;

use rune::{Env, Runtime};

use super::*;
use crate::app::{AppState, Job};
use crate::ui::view;

fn pic() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/bg.jpg")
}

fn jobs(s: &AppState) -> usize {
    s.jobs.iter().filter(|j| matches!(j, Job::Background { .. })).count()
}

fn tick_with(s: &mut AppState) {
    let mut tx = rune::Tx::default();
    tick(s, 0.0, &mut tx);
}

#[test]
fn tick_queues_one_job_per_path_change() {
    let mut s = AppState::new();
    tick_with(&mut s);
    assert_eq!(jobs(&s), 0);
    s.settings.background.path = Some(pic());
    tick_with(&mut s);
    tick_with(&mut s);
    assert_eq!(jobs(&s), 1);
    s.settings.background.path = None;
    tick_with(&mut s);
    assert_eq!(jobs(&s), 1, "clearing the path queues nothing");
    assert!(s.bg.image.is_none() && s.bg.for_path.is_none());
}

#[test]
fn stale_result_is_dropped() {
    let mut s = AppState::new();
    s.settings.background.path = Some(pic());
    tick_with(&mut s);
    let r = crate::worker::run_job(s.jobs.remove(0));
    s.settings.background.path = Some(PathBuf::from("/other.jpg"));
    tick_with(&mut s);
    r.apply(&mut s);
    assert!(s.bg.image.is_none());
}

#[test]
fn failed_decode_sets_flag_and_toast() {
    let mut s = AppState::new();
    s.settings.background.path = Some(PathBuf::from("/no/such.jpg"));
    tick_with(&mut s);
    crate::worker::run_job(s.jobs.remove(0)).apply(&mut s);
    assert!(s.bg.failed && s.bg.image.is_none());
    assert_eq!(s.toast.as_ref().map(|t| t.text.as_str()), Some("Couldn't load the background picture"));
}

fn runtime(s: AppState) -> Runtime<AppState> {
    Runtime::new(s, view, (1100.0, 760.0), Env { system_dark: false, scale: 2.0 })
}

#[test]
fn root_has_bg_image_only_when_ready() {
    let mut s = AppState::new();
    s.settings.background.path = Some(pic());
    let mut rt = runtime(s);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("bg-image").is_none());

    let mut s = AppState::new();
    s.settings.background.path = Some(pic());
    tick_with(&mut s);
    crate::worker::run_job(s.jobs.remove(0)).apply(&mut s);
    assert!(s.bg.image.is_some());
    let mut rt = runtime(s);
    rt.frame(1.0 / 60.0);
    assert!(rt.find("bg-image").is_some());
}
