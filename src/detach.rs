//! Started from a terminal, Marko gives the terminal back, as `code .` does: a release build
//! starts itself again in its own session, detached from the terminal, and the first process
//! exits. The window is the same either way; only who waits for it changes.
//!
//! It stays in the foreground for `--wait`, for development builds (`cargo run` keeps its logs),
//! for Rune's dev flags (`--snap`, `--list-scenes` and the like print or write files), and when
//! nothing is a terminal (a menu or file manager launch has nobody waiting).

/// The variable the background copy is started with, so it doesn't detach again.
const DETACHED: &str = "MARKO_DETACHED";
/// The flag that keeps Marko in the foreground. `extra_args` callers drop it (`is_flag`).
pub const WAIT: &str = "--wait";

/// Whether this launch should hand itself to a background copy. `args` excludes the program.
pub fn should_detach(args: &[String], terminal: bool, already_detached: bool, release: bool) -> bool {
    release && terminal && !already_detached && !args.iter().any(|a| a.starts_with("--"))
}

/// Whether `arg` is Marko's own flag rather than a path to open.
pub fn is_flag(arg: &str) -> bool {
    arg == WAIT
}

/// Detaches when `should_detach` says so: starts the background copy and exits this process.
/// Returns (and Marko runs in the foreground) when it shouldn't, or the copy can't start.
#[cfg(unix)]
pub fn maybe_detach() {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let args: Vec<String> = std::env::args().skip(1).collect();
    // SAFETY: isatty only inspects the descriptor.
    let terminal = unsafe { libc::isatty(0) == 1 || libc::isatty(1) == 1 || libc::isatty(2) == 1 };
    if !should_detach(&args, terminal, std::env::var_os(DETACHED).is_some(), !cfg!(debug_assertions)) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else { return };
    let mut cmd = Command::new(exe);
    cmd.args(std::env::args_os().skip(1))
        .env(DETACHED, "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // SAFETY: setsid is async-signal-safe and runs in the child between fork and exec. A new
    // session has no controlling terminal, so closing the terminal doesn't close the window.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    if cmd.spawn().is_ok() {
        std::process::exit(0);
    }
}

/// Windows release builds are GUI programs: a terminal doesn't wait for them.
#[cfg(not(unix))]
pub fn maybe_detach() {}

#[cfg(test)]
#[path = "detach_tests.rs"]
mod tests;
