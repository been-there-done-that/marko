# Lightness budgets

Budgets come from PLAN.md §10. Numbers are from one release build (`cargo build --release`,
thin LTO, stripped) on the Linux development laptop (Wayland session, 16 logical CPUs), with
`fixtures/basic.md` open. Each row says what the number leaves out.

| Metric | Budget (PLAN.md §10) | Measured | What the number leaves out |
|---|---|---|---|
| Cold start to first content frame (1 small file) | ≤ 250 ms | Not measured (see below) | — |
| Idle CPU, document open, no input | 0% (no frames drawn) | 0 utime+stime ticks over 20 s (CLK_TCK 100, so 0 ms CPU) | One 20 s window, one fixture, no pointer or focus events. The Rune frame counter was not read, so "no frames drawn" is inferred from zero CPU, not counted. |
| Memory footprint, 1 typical README | ≤ 90 MB | VmRSS 63.4 MB at 3 s with `fixtures/basic.md` (was 106.9 MB before `src/gpu_env.rs` stopped the Vulkan loader from loading the LLVM-linked lavapipe and radv drivers) | Measured with `/proc/<pid>/status` on the Linux build only. The fixture is small, so this is a floor for a typical README. Excludes GPU/driver memory and the wgpu device allocations that are not in the process RSS. procwatch was not used. |
| Memory, 20 tabs + 2 roots of ~2,000 files | ≤ 160 MB | Not measured | Needs 20 open tabs and two sidebar roots of about 2,000 files each. Not set up in this pass. |
| Tab switch (warm) | next frame (≤ 8 ms at 120 Hz) | Not measured | Needs a frame profile of a running window. |
| Scroll a 10k-line doc | holds 120 Hz on ProMotion | Not measured | Needs a 10k-line fixture and a bench run; no such fixture exists in `fixtures/`. |
| Live reload of a 200 KB file | visible ≤ 100 ms after save | Not measured | Needs a timed save and a screen capture. |
| Release binary (stripped, LTO) | ≤ 25 MB per OS | 27,275,672 bytes (26.0 MB) on Linux, from `ls -l` | Linux x86-64 only. 1 MB over budget. `opt-level = "s"`, `lto = "fat"` and `codegen-units = 1` brought it down from 42.2 MB; the last megabyte would need `opt-level = "z"` or dropping a feature such as Mermaid. |

## Cold start

Not measured. Rune's launch timing (`crates/rune/src/tooling/perf_warn/launch.rs`) is compiled
only under `cfg(debug_assertions)` and reports only when the first frame exceeds its warning
threshold, so a release build prints no timing. There is no `RUNE_FLIGHT` variable; the
`flight` module in Rune is for animation flights, not startup. Measuring cold start needs
either a debug build or a timestamp taken in the app's `on_ready` against process start, which
this pass did not add.

## How the numbers were taken

- Binary: `ls -l` on the release binary in the shared target directory.
- RSS: start the release binary with `fixtures/basic.md` in the background, wait 3 s, read
  `VmRSS` from `/proc/<pid>/status`.
- Idle CPU: read `utime + stime` (fields 14 and 15 of `/proc/<pid>/stat`) at the start and
  after 20 s with no input; the difference is the idle tick count. The process was then stopped.
- Each number is one run, not a median of paired runs. Treat them as first readings.
