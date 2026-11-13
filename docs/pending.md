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

Remaining, in value order:

1. **Scaled JPEG decode.** JPEG's DCT can be decoded at 1/2, 1/4 or 1/8 scale, which cuts the
   peak 4–64× for a large photo. `image` 0.25 does not expose it; `zune-jpeg`'s `set_max_*`
   only **rejects**, it does not downscale. The path is a direct `jpeg-decoder` dependency (it
   has `scale()`) or upstreaming scaling into `image`. Colour-space (CMYK/YCCK) and EXIF
   orientation handling come with it.
2. **Lazy image decoding.** Today every image in the open document is queued on load. Decoding
   only the images within about one viewport of the visible area would cut CPU and the transient
   peak for image-heavy documents. The Markdown image callback already runs during layout and
   knows the visible range.
3. **Peak is still ~2× the cap.** A 16-megapixel photo can allocate 64 MB in the decoder plus
   up to 64 MB for RGBA. Lowering `MAX_DECODE_ALLOC` trades support for peak; 64 MB was chosen
   so a 12–16 MP phone photo still opens.
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
