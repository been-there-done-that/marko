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

Done in this pass:

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

Remaining, in value order:

1. **Lazy image decoding.** Today every image in the open document is queued on load. Decoding
   only the images within about one viewport of the visible area would cut CPU and the transient
   peak for image-heavy documents. The Markdown image callback already runs during layout and
   knows the visible range.
2. **Cap concurrent decodes.** Local and web decodes run on separate workers and can peak
   together. A semaphore would bound the worst case to one image's buffers.
3. **Peak for non-JPEGs is still ~2× the cap.** PNG/WebP cannot be scaled during decode, so a
   16-megapixel PNG can allocate 64 MB in the decoder plus up to 64 MB for RGBA. Lowering
   `MAX_DECODE_ALLOC` trades support for peak; 64 MB lets a 12–16 MP photo open.
4. **Measure it.** The budget doc has no image-heavy fixture. Add one (a README with several
   large photos) and record peak RSS with `/proc/<pid>/status` sampled during decode.

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
