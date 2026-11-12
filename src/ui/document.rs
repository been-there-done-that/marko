//! The document area (§4.7, §4.12): the active tab's Markdown in a centred reading column, or
//! its error, with the large-file and lossy-decode banners above it.

use std::ops::Range;
use std::path::{Path, PathBuf};

use rune::{El, ScrollSpec, TextStyle, Tx, col, el, icon, row, text};
use rune_kit::markdown::CodeFrame;
use rune_kit::{Button, Highlight, Kit, Markdown};

use crate::app::AppState;
use crate::load::{DocState, LoadError, strip_frontmatter};
use crate::icons;
use crate::images::{Resolved, resolve, web_key};
use crate::ui::lightbox;
use crate::paths::is_markdown;
use crate::progress::{self, doc_key};
use crate::theme::{Look, self, READ_W, READ_W_WIDE, Tokens};

/// Side padding of the reading column, top and bottom padding (§4.7).
const SIDE_PAD: f32 = 40.0;
const TOP_PAD: f32 = 40.0;
const BOTTOM_PAD: f32 = 96.0;

/// The document area for the active tab. `Loading` shows nothing for its one frame.
pub fn view(s: &AppState, k: &Kit, look: &'static Look) -> El {
    let t = &look.tokens;
    let Some(tab) = s.tabs.active() else { return el().key("doc") };
    let ready = matches!(tab.doc, DocState::Ready { .. });
    let content = match &tab.doc {
        DocState::Loading => el(),
        DocState::Error(e) => error(&tab.path, e, k, t),
        DocState::Ready { src, large, lossy } => {
            let mut c = col().w_full();
            if tab.deleted {
                c = c.child(deleted_banner(k, t));
            }
            if *large {
                c = c.child(banner("doc-banner-large", "Large file — some features are slower", k, t));
            }
            if *lossy {
                c = c.child(banner("doc-banner-lossy", "Some characters couldn't be read", k, t));
            }
            let blocked = s.blocked_web_images(tab.id);
            if blocked > 0 {
                c = c.child(web_banner(tab.id, blocked, k, t));
            }
            let md = markdown(
                s,
                k,
                look,
                MarkdownDoc {
                    key: &format!("doc-{}", tab.id),
                    rev: tab.rev,
                    src,
                    path: &tab.path,
                    highlights: find_highlights(s, tab.id),
                },
            );
            // A deleted file keeps its last text, dimmed to 60%.
            c.child(if tab.deleted { md.layer_opacity(0.6) } else { md })
        }
    };
    let scroll = reading(&doc_key(tab.id, ready), s, content).on_scroll_offset(progress::on_report(tab, &s.doc_scroll));
    let scroll = if ready { scroll.on_visible(progress::on_show(tab.id)) } else { scroll };
    with_progress(scroll, s, k, t)
}

/// The document's scroll viewport with the reading-progress line across its top and the
/// percentage pill at its bottom right. Both are in the tree only while the document scrolls.
fn with_progress(scroll: El, s: &AppState, k: &Kit, t: &Tokens) -> El {
    let area = col().key("doc-area").w_full().grow(1.0).min_h(0.0).child(scroll);
    if !s.doc_scroll.shown || !s.settings.show_progress {
        return area;
    }
    let line = el().key("doc-progress-line").pin(Some(0.0), Some(0.0), Some(0.0), None).h(progress::LINE_H).fill(t.accent)
        .live_rect(&s.doc_scroll.line);
    let pill = row().key("doc-progress").pin(None, None, Some(16.0), Some(16.0)).h(22.0).px(8.0).center_y().radius(11.0)
        .fill(t.code_bg).border(1.0, t.code_line)
        .child(text("100%").font(k.ty.small).color(t.fg3).live_text(&s.doc_scroll.pct));
    area.child(line).child(pill)
}

/// The inputs to one rendered Markdown tree.
pub struct MarkdownDoc<'a> {
    /// Stable per-view key, unique across the tabs.
    pub key: &'a str,
    /// Bumped whenever the shown text changes.
    pub rev: u64,
    pub src: &'a str,
    pub path: &'a Path,
    pub highlights: Vec<(Vec<Range<usize>>, Highlight)>,
}

/// The rendered Markdown for `src`, with its frontmatter hidden. Links and images resolve against
/// the folder of `path`. Images are read from the app's cache; one not decoded yet (or that
/// failed) shows the broken-image glyph and its alt text. The Edit view's preview renders its
/// buffer's text through this too, with no find highlights, since the matches were found in the
/// saved source.
///
/// The tree is memoised by `key` and `rev`: Rune rebuilds it only when the revision, the style,
/// the image cache or the highlights change, not on every frame. Rune's own cache cannot help
/// because the image and code callbacks disable it.
pub fn markdown(s: &AppState, k: &Kit, look: &'static Look, doc: MarkdownDoc<'_>) -> El {
    let version = markdown_version(s, look, doc.key, doc.rev, doc.path, &doc.highlights);
    rune::memo::memo_with_hash(format!("md-{}", doc.key), version, move || build_markdown(s, k, look, doc))
}

/// Everything the rendered Markdown depends on, cheaply hashed: the look's identity, the text
/// settings, the tab's revision, the image cache and the find highlights.
fn markdown_version(
    s: &AppState,
    look: &'static Look,
    key: &str,
    rev: u64,
    path: &Path,
    highlights: &[(Vec<Range<usize>>, Highlight)],
) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h = DefaultHasher::new();
    (look as *const Look as usize).hash(&mut h);
    s.settings.text_size.to_bits().hash(&mut h);
    s.settings.line_spacing.to_bits().hash(&mut h);
    s.settings.ligatures.hash(&mut h);
    s.settings.body_font.hash(&mut h);
    s.settings.code_font.hash(&mut h);
    s.zoom.to_bits().hash(&mut h);
    key.hash(&mut h);
    rev.hash(&mut h);
    path.hash(&mut h);
    s.images.version().hash(&mut h);
    s.find.open.hash(&mut h);
    s.find.current.hash(&mut h);
    for (ranges, _) in highlights {
        for r in ranges {
            r.start.hash(&mut h);
            r.end.hash(&mut h);
        }
    }
    h.finish()
}

fn build_markdown(s: &AppState, k: &Kit, look: &'static Look, doc: MarkdownDoc<'_>) -> El {
    let MarkdownDoc { src, path, highlights, .. } = doc;
    let doc = path.to_path_buf();
    let link_doc = doc.clone();
    let hits = s.images.hits();
    Markdown::new(strip_frontmatter(src))
        .style(theme::markdown_style(look, &s.settings, s.zoom))
        .scroll_wide(true)
        .selectable(true)
        .tasks_read_only()
        .highlights(highlights)
        .images(move |src: &str| match resolve(&doc, src) {
            Resolved::Local(p) => hits.get(&p).cloned(),
            // A web image not fetched (not allowed, or not arrived) is absent, so it shows its alt text.
            Resolved::Remote(url) => hits.get(&web_key(&url)).cloned(),
            Resolved::Unsupported => None,
        })
        .image_frame(lightbox::open_on_press)
        .code_frame(move |card, f| code_header(card, f, look))
        .on_link_tx(move |s: &mut AppState, url: &str, tx: &mut Tx| route_link(s, &link_doc, url, tx))
        .link_help(|href: &str| Some(href.to_string()))
        .build(k)
}

/// A code block with a header row: its language on the left, a Copy button on the right.
fn code_header(card: El, f: &CodeFrame<'_>, look: &'static Look) -> El {
    let k = &look.kit;
    let t = &look.tokens;
    let code = f.source.to_string();
    let copy = Button::new("Copy")
        .on_press_tx(move |_: &mut AppState, tx: &mut Tx| tx.copy(code.clone()))
        .build(k)
        .key(format!("code-copy-{}", f.index));
    let head = row().w_full().h(32.0).px(12.0).center_y().fill(t.code_bg)
        .child(text(f.language.unwrap_or("text")).font(k.ty.small).color(t.fg2))
        .child(el().grow(1.0))
        .child(copy);
    col().w_full().child(head).child(card)
}

/// A scrolling reading column keyed `key`, centred, holding `content`.
pub fn reading(key: &str, s: &AppState, content: El) -> El {
    let width = if s.wide { READ_W_WIDE } else { READ_W };
    let column = col().w_full().max_w(width + 2.0 * SIDE_PAD).px(SIDE_PAD).pt(TOP_PAD).pb(BOTTOM_PAD).child(content);
    let centred = row().w_full().justify_center().child(column);
    col().key(key).w_full().grow(1.0).min_h(0.0).scroll(ScrollSpec { stick_bottom: false }).child(centred)
}

/// The find matches for `tab`, with the current one stronger. Empty unless the bar is open and its
/// matches were found in this tab's source (`Find::for_tab`).
fn find_highlights(s: &AppState, tab: u64) -> Vec<(Vec<Range<usize>>, Highlight)> {
    if !s.find.open || s.find.for_tab != Some(tab) {
        return Vec::new();
    }
    s.find.matches.iter().enumerate()
        .map(|(i, m)| (m.clone(), if i == s.find.current { Highlight::current() } else { Highlight::new() }))
        .collect()
}

/// A link in the document. Web and mail links open in the browser. A `#anchor` does nothing yet.
/// A relative Markdown file opens as a tab; any other local file opens in the system's app for it.
fn route_link(s: &mut AppState, doc: &Path, url: &str, tx: &mut Tx) {
    if ["http:", "https:", "mailto:"].iter().any(|scheme| url.starts_with(scheme)) {
        let _ = tx.open_url(url);
        return;
    }
    let Resolved::Local(target) = resolve(doc, url) else { return };
    if is_markdown(&target) {
        let now = s.now;
        s.open_paths(&[target], now);
    } else {
        open_with_os(&target);
    }
}

/// Opens a file in the system's default app. `tx.open_url` refuses `file:` links, so this runs
/// the platform opener on its own thread, and the UI thread never waits on it.
pub(crate) fn open_with_os(path: &Path) {
    let path = path.to_path_buf();
    let opener = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "explorer"
    } else {
        "xdg-open"
    };
    std::thread::spawn(move || {
        let _ = std::process::Command::new(opener).arg(&path).status();
    });
}

/// The notice for a file deleted on disk, with a button that closes its tab.
fn deleted_banner(k: &Kit, t: &Tokens) -> El {
    let close = Button::new("Close Tab")
        .on_press(|s: &mut AppState| {
            if let Some(id) = s.tabs.active().map(|tab| tab.id) {
                s.request_close(id);
            }
        })
        .build(k)
        .key("deleted-close");
    row().key("doc-banner-deleted").w_full().mb(16.0).px(12.0).py(8.0).radius(6.0).fill(t.code_bg).border(1.0, t.code_line).center_y()
        .child(text("This file was deleted").font(k.ty.small).color(t.fg2))
        .child(el().grow(1.0))
        .child(close)
}

/// Above a document whose web images are not allowed to load: what it holds, and a Load button.
/// Loading an image tells its server the file was opened, so the reader decides.
fn web_banner(tab: u64, count: usize, k: &Kit, t: &Tokens) -> El {
    let noun = if count == 1 { "web image" } else { "web images" };
    let load = Button::new("Load")
        .on_press(move |s: &mut AppState| s.load_web_images(tab))
        .build(k)
        .key("doc-web-load");
    row().key("doc-banner-web").w_full().mb(16.0).px(12.0).py(8.0).radius(6.0).fill(t.code_bg).border(1.0, t.code_line).center_y()
        .child(text(format!("This document has {count} {noun}")).font(k.ty.small).color(t.fg2))
        .child(el().grow(1.0))
        .child(load)
}

fn banner(key: &str, msg: &str, k: &Kit, t: &Tokens) -> El {
    row().key(key).w_full().mb(16.0).px(12.0).py(8.0).radius(6.0).fill(t.code_bg).border(1.0, t.code_line).center_y()
        .child(text(msg).font(k.ty.small).color(t.fg2))
}

/// A tab that could not be read: the file name, and why.
fn error(path: &Path, e: &LoadError, k: &Kit, t: &Tokens) -> El {
    let name: PathBuf = path.file_name().map(PathBuf::from).unwrap_or_else(|| path.to_path_buf());
    let title = TextStyle { size: 17.0, weight: 650.0, ..k.ty.body };
    col().key("doc-error").w_full().pt(64.0).center().gap(8.0)
        .child(icon(&icons::FILE, 32.0).color(t.fg3))
        .child(text(format!("Can't open {}", name.display())).font(title).color(t.fg))
        .child(text(e.message()).font(k.ty.small).color(t.fg3).wrap())
        .child(retry_button(k))
}

/// Queues the active tab's load again (§4.12).
fn retry_button(k: &Kit) -> El {
    Button::new("Try Again")
        .on_press(|s: &mut AppState| s.requeue_active_load())
        .build(k)
        .key("doc-retry")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    #[test]
    fn markdown_version_changes_with_revision_and_key() {
        let s = AppState::new();
        let look = theme::look(&Settings::default(), &[], false);
        let path = Path::new("/d/a.md");
        let a = markdown_version(&s, look, "doc-1", 1, path, &[]);
        assert_eq!(a, markdown_version(&s, look, "doc-1", 1, path, &[]), "stable for the same inputs");
        assert_ne!(a, markdown_version(&s, look, "doc-1", 2, path, &[]), "a new revision rebuilds");
        assert_ne!(a, markdown_version(&s, look, "doc-2", 1, path, &[]), "a different tab rebuilds");
    }
}
