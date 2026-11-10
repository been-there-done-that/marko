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
use crate::images::{Resolved, resolve};
use crate::paths::is_markdown;
use crate::theme::{self, READ_W, READ_W_WIDE, Tokens};

/// Side padding of the reading column, top and bottom padding (§4.7).
const SIDE_PAD: f32 = 40.0;
const TOP_PAD: f32 = 40.0;
const BOTTOM_PAD: f32 = 96.0;

/// The document area for the active tab. `Loading` shows nothing for its one frame.
pub fn view(s: &AppState, k: &Kit, dark: bool) -> El {
    let t = theme::tokens(dark);
    let Some(tab) = s.tabs.active() else { return el().key("doc") };
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
            let md = markdown(s, k, dark, src, &tab.path, find_highlights(s, tab.id));
            // A deleted file keeps its last text, dimmed to 60%.
            c.child(if tab.deleted { md.layer_opacity(0.6) } else { md })
        }
    };
    reading("doc", s, content)
}

/// The rendered Markdown for `src`, with its frontmatter hidden. Links and images resolve against
/// the folder of `path`. Images are read from the app's cache; one not decoded yet (or that
/// failed) shows the broken-image glyph and its alt text. The Edit view's preview renders its
/// buffer's text through this too, with no find highlights, since the matches were found in the
/// saved source.
pub fn markdown(s: &AppState, k: &Kit, dark: bool, src: &str, path: &Path, highlights: Vec<(Vec<Range<usize>>, Highlight)>) -> El {
    let doc = path.to_path_buf();
    let link_doc = doc.clone();
    let hits = s.images.hits();
    Markdown::new(strip_frontmatter(src))
        .style(theme::markdown_style(dark, s.zoom))
        .scroll_wide(true)
        .selectable(true)
        .tasks_read_only()
        .highlights(highlights)
        .images(move |src: &str| match resolve(&doc, src) {
            Resolved::Local(p) => hits.get(&p).cloned(),
            // TODO(upstream): "Remote image · Load" needs a placeholder the Markdown image hook can draw.
            Resolved::Remote(_) | Resolved::Unsupported => None,
        })
        .code_frame(move |card, f| code_header(card, f, dark))
        .on_link_tx(move |s: &mut AppState, url: &str, tx: &mut Tx| route_link(s, &link_doc, url, tx))
        .link_help(|href: &str| Some(href.to_string()))
        .build(k)
}

/// A code block with a header row: its language on the left, a Copy button on the right.
fn code_header(card: El, f: &CodeFrame<'_>, dark: bool) -> El {
    let k = theme::kit(dark);
    let t = theme::tokens(dark);
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
fn open_with_os(path: &Path) {
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
                s.tabs.close(id);
            }
        })
        .build(k)
        .key("deleted-close");
    row().key("doc-banner-deleted").w_full().mb(16.0).px(12.0).py(8.0).radius(6.0).fill(t.code_bg).border(1.0, t.code_line).center_y()
        .child(text("This file was deleted").font(k.ty.small).color(t.fg2))
        .child(el().grow(1.0))
        .child(close)
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
}
