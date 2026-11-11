# Contributing to Marko

Thanks for helping. Pull requests are welcome, from one-line fixes to new features. You don't need to open an issue first. For a large change, such as a new mode, a new dependency or a different layout, opening an issue to talk it through first can save you rework.

## Reporting a bug

Open an issue with:

- what you did, what you expected and what happened instead;
- your OS and desktop (for example "Ubuntu 24.04, GNOME on Wayland"), and Marko's version or commit;
- a Markdown file that shows the problem, if it's about rendering;
- a screenshot or short screen recording for anything visual.

## Making a change

1. Fork the repo and create a branch from `main`.
2. Build and run it:

   ```sh
   cargo run -- README.md
   ```

   On Linux you need `pkg-config`, `libfontconfig1-dev`, `libxkbcommon-dev` and `libwayland-dev`.

3. Before you push, run what CI runs:

   ```sh
   cargo clippy --all-targets --locked -- -D warnings
   cargo test --locked
   ```

4. Open a pull request against `main`.

## What a good pull request has

- **One change.** A fix and an unrelated cleanup are two pull requests.
- **Tests.** Most of Marko's logic has unit tests beside it, in a sibling `*_tests.rs` file (`tabs.rs` → `tabs_tests.rs`). The UI can be tested headlessly with `rune::Runtime`, as `src/ui/titlebar_tests.rs` does: build the view, run a frame and check where elements landed.
- **A screenshot for anything visual.** Render one headlessly with a built-in scene, so it looks the same on every machine:

  ```sh
  cargo run -- --snap out.png --scene folder-dark --theme dark
  cargo run -- --list-scenes
  ```

- **A clear commit message.** The subject names the area, then says what changed, in the present tense: `Find: Enter and Escape keep working once the field has focus`. Use the body for the reason when it isn't obvious.

## How Marko is built

- **Rust, on the [Rune](https://github.com/anomalyco/rune) GPU UI framework.** Views are plain functions from state to elements (`src/ui/`). App state lives in `AppState` (`src/app.rs`).
- **Nothing slow on the UI thread.** Loading files, scanning folders, watching for changes and writing settings all run on worker threads (`src/worker.rs`, `src/watch.rs`, `src/settings_sync.rs`).
- **Idle means idle.** A window that isn't changing draws nothing and uses no CPU. A change that makes Marko redraw while nothing moves is a bug; the memory and CPU readout in the toolbar (Settings → General) shows it.
- **Bounded caches.** Image caches are limited in both entries and bytes, counting decoded pixels. Keep new caches the same way.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE), like the rest of Marko.
