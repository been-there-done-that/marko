# Pending work

Deliberately left open, with enough context to pick each up. Newest first is not the order;
these are independent.

## Architecture: `AppState` is a god object

- About 39 fields, and three places that dispatch commands: `ui/keys.rs`, `commands.rs`, and the
  menu closures. Adding one command touches several files.
- `tabs.activate` and `sync_find` must be called together; it is easy to miss one.
- `cargo fmt --check` is red across the existing tree. Do a format-only commit first, with no
  behaviour change, so the refactor's diff stays readable.
- Direction: give `AppState` one `activate_tab(id)` (and one `close_tab`) that always updates
  find, scroll, and the edit buffer, so callers cannot forget. Then move the sidebar, document,
  and job state behind small structs. Keep behaviour identical; the 249 tests are the harness.
- Deferred because it is wide and risky, and it is not a bug. It belongs in its own PR.

## DNS rebinding (SSRF residual)

`net.rs::host_is_public` resolves a hostname and checks the address, but `ureq` resolves it again
when it connects. A DNS server the attacker controls can answer with a public address for the
check and a private one for the fetch.

- `ureq`'s public API does not expose the resolved socket, so the address cannot be pinned.
- Options: a custom resolver on the agent, a client that lets us connect to a chosen IP while
  sending the original `Host`, or a small HTTP client we own.
- Limited today: web images are fetched only after the user allows them, and only image bytes
  reach the renderer. Still worth closing.
- The reason is documented at `host_is_public`.

## Decode peak and memory

Done:

- The header is read first and a picture whose decoded RGBA would exceed 64 MB is refused
  **before** the decoder allocates (`too_big`). `image`'s `max_alloc` is advisory, so this check
  is the real bound.
- `fit_with` scales by the smaller of the width cap and the cache's pixel budget, so a tall,
  narrow image is shrunk rather than decoded at full size and rejected afterwards.
- Decoded-RGBA cache 48 MB → 32 MB; background 3840 px → 2560 px wide.
- Strict `max_image_width`/`max_image_height` caps (16,384) as a second line of defence.
- JPEGs take a fast path through `jpeg-decoder`, which asks the DCT for a 1/8, 1/4 or 1/2 scale
  output when the picture is larger than the column. A 100 MP photo at 1/4 no longer allocates
  its full-resolution buffer. CMYK, 16-bit and lossless JPEGs return `None` and fall back to
  `image`, which handles them.
- Only the active tab's images around its scroll position (plus the first window) are queued.
  An inactive tab queues nothing until it is shown, the editor preview uses its own list, and
  web images use the same window.
- At most 4 image decodes are in flight (queued, decoding, or decoded but not yet applied), so
  results cannot pile up in the channel faster than the UI applies them. A failed decode moves
  to a `failed` set, which frees the slot and stops retries until the file changes.
- A stable window is decoded once. A path decoded since it entered the window is not queued
  again even if the cache evicts it; a path new to the window has that guard cleared, so
  scrolling re-decodes what was evicted.
- One decode at a time across the local and web workers (`DECODE_LOCK`), so two large pictures
  cannot double the transient peak.

Remaining:

1. **Peak for non-JPEGs is still ~4–5× the RGBA cap.** The image-heavy fixture measured
   266.9 MB HWM against a 48.7 MB baseline. `image`'s PNG decoder allocates a full-resolution
   frame regardless of `max_alloc`; its own `set_limits` carries a TODO because it cannot
   constrain the `png` crate's internal buffers. Streaming PNG decode row by row, with a box or
   nearest downscale into the target, would remove the 48 MB frame and is the next real win.
   Lowering `MAX_DECODE_ALLOC` only refuses more pictures.
2. **The window is a heuristic.** It assumes images are spread through the document in order.
   A document with one huge image followed by many small ones can still show alt text for an
   image that is on screen but outside the index window. The first window and the cache make
   this rare; a real fix needs layout positions from the renderer.

## Artifact signing and notarization

- Needs an Apple Developer ID, a Windows code-signing certificate, and CI secrets.
- The installers already verify `SHA256SUMS` over HTTPS and the README tells macOS users how to
  clear quarantine after that check.
- When secrets exist: sign the macOS `.app`/`.dmg`, sign the Windows `.exe`, and add
  `gh attestation`/SLSA provenance for the archives.

## Performance measurements still missing

`docs/budgets.md` lists cold start, warm tab switch, a 10k-line scroll, live reload, and 20 tabs
plus two 2,000-file roots as "not measured". They need fixtures (a 10k-line document, two large
roots) and a way to time a release build; Rune's launch timing is debug-only. Measure before
optimising further.
