//! Reading progress for the document area. Each tab remembers its scroll offset: it is copied out
//! of the viewport while the tab is shown, restored when the tab is shown again, and saved with
//! the session. The active document shows an accent line and a percentage pill. The viewport
//! reports into `Live` values, so scrolling moves the line and the text without rebuilding the view.

use rune::{Live, LiveText, ScrollPos, Tx};

use crate::app::{AppState, Mode};
use crate::load::DocState;
use crate::tabs::Tab;

/// Height of the reading-progress line, in points.
pub const LINE_H: f32 = 2.0;

/// The key of a tab's document scroll viewport. A Ready document keeps `doc-{id}`. Any other state
/// gets its own key, so the viewport is mounted anew when the document arrives and its saved
/// offset is applied then.
pub fn doc_key(id: u64, ready: bool) -> String {
    if ready { format!("doc-{id}") } else { format!("doc-{id}-idle") }
}

/// A tab's viewport report: offset, maximum and viewport width, all on y. `max` is -1 until the
/// viewport has reported, so nothing reads a stale value from before it mounted.
pub fn tab_live() -> Live<[f32; 4]> {
    Live::new([0.0, -1.0, 0.0, 0.0])
}

/// Reading-progress state for the active document.
pub struct Progress {
    /// Presentation rect of the accent line, `[x, y, w, h]` in the document area.
    pub line: Live<[f32; 4]>,
    /// The percentage text inside the pill.
    pub pct: LiveText,
    /// The viewport key whose saved offset was last applied. Copies out of the viewport wait for
    /// it, so a freshly mounted viewport's first report (offset 0) cannot overwrite the saved one.
    pub restored: Option<String>,
    /// True when the line and pill are in the view: the active document is ready and scrolls.
    pub shown: bool,
}

impl Progress {
    pub fn new() -> Self {
        Self { line: Live::new([0.0; 4]), pct: LiveText::new("0%"), restored: None, shown: false }
    }
}

/// How far down the document `offset` is, from 0 to 1, given the largest offset `max`.
fn fraction(offset: f32, max: f32) -> f32 {
    if max > 0.0 { (offset / max).clamp(0.0, 1.0) } else { 0.0 }
}

/// The viewport's `on_scroll_offset` handler for tab `tab` (its live report), which also moves the
/// progress line and text. Only the active tab's viewport is mounted, so only it drives them.
pub fn on_report(tab: &Tab, p: &Progress) -> impl Fn(ScrollPos) + 'static {
    let at = tab.scroll_at.clone();
    let line = p.line.clone();
    let pct = p.pct.clone();
    move |pos: ScrollPos| {
        let [offset, max] = [pos.offset[1], pos.max[1]];
        let viewport_w = pos.viewport[0];
        at.set([offset, max, viewport_w, 0.0]);
        let frac = fraction(offset, max);
        line.set([0.0, 0.0, viewport_w * frac, LINE_H]);
        pct.set(format!("{}%", (frac * 100.0).round() as u32));
    }
}

/// The handler a Ready viewport runs when it comes on screen. A viewport mounts on screen (a tab
/// shown, a document loaded), so this applies tab `id`'s saved offset then. It runs again when the
/// viewport leaves and returns, and then does nothing, since `Progress::restored` already names it.
pub fn on_show(id: u64) -> impl Fn(&mut AppState, bool, &mut Tx) + 'static {
    move |s: &mut AppState, on: bool, tx: &mut Tx| {
        tx.live_only();
        if !on {
            return;
        }
        let key = doc_key(id, true);
        if s.doc_scroll.restored.as_deref() == Some(key.as_str()) {
            return;
        }
        s.doc_scroll.restored = Some(key.clone());
        let saved = s.tabs.iter().find(|t| t.id == id).map_or(0.0, |t| t.scroll);
        if saved > 0.0 {
            tx.scroll_to_instant(key, 0.0, saved);
        }
    }
}

impl AppState {
    /// Once per frame, from `poll`. Copies the active document's offset into its tab while its
    /// viewport is the one restored. Returns true when the progress line and pill show or hide,
    /// so the view runs again.
    pub fn sync_doc_scroll(&mut self) -> bool {
        // Edit mode has no document viewport, so the next Preview mounts one that needs restoring.
        if matches!(self.mode, Mode::Edit) {
            self.doc_scroll.restored = None;
        }
        let Some(tab) = self.tabs.active_mut() else { return self.set_progress_shown(false) };
        let ready = matches!(tab.doc, DocState::Ready { .. });
        let [offset, max, _, _] = tab.scroll_at.get();
        let restored = self.doc_scroll.restored.as_deref() == Some(doc_key(tab.id, true).as_str());
        if ready && restored && max >= 0.0 {
            tab.scroll = offset;
        }
        let shown = ready && max > 0.0;
        self.set_progress_shown(shown)
    }

    fn set_progress_shown(&mut self, shown: bool) -> bool {
        let changed = self.doc_scroll.shown != shown;
        self.doc_scroll.shown = shown;
        changed
    }
}
