//! Find in the active document (§4.8): the query, its matches and the current one.
//!
//! Matches are byte ranges into the source the document view renders, which is the source
//! with its frontmatter stripped. `AppState::sync_find` keeps them in step with the active tab.

use std::ops::Range;

use rune::{TextEdit, Tx};
use rune_kit::markdown::{Case, find};
use rune_kit::Markdown;

use crate::app::AppState;
use crate::load::{DocState, strip_frontmatter};

/// The find bar's state. `matches` holds one entry per block match, in document order.
pub struct Find {
    pub open: bool,
    pub query: TextEdit,
    pub matches: Vec<Vec<Range<usize>>>,
    pub current: usize,
    /// The tab whose source `matches` were found in. The document only paints matches for it.
    pub for_tab: Option<u64>,
}

impl Find {
    pub fn new() -> Self {
        Self { open: false, query: TextEdit::new(""), matches: Vec::new(), current: 0, for_tab: None }
    }

    /// Finds the query in `src` (case-insensitive) and keeps `current` in range.
    pub fn run(&mut self, src: &str) {
        self.matches = find(src, &self.query.text, Case::Insensitive);
        self.current = self.current.min(self.matches.len().saturating_sub(1));
    }

    /// Moves to the next match, wrapping from the last to the first.
    pub fn next(&mut self) {
        let n = self.matches.len();
        if n > 0 {
            self.current = (self.current + 1) % n;
        }
    }

    /// Moves to the previous match, wrapping from the first to the last.
    pub fn prev(&mut self) {
        let n = self.matches.len();
        if n > 0 {
            self.current = (self.current + n - 1) % n;
        }
    }

    /// Total number of matches.
    pub fn count(&self) -> usize {
        self.matches.len()
    }

    /// `"3 of 12"`, `"No results"` for a query with no match, and `""` for an empty query.
    pub fn label(&self) -> String {
        if self.query.text.is_empty() {
            String::new()
        } else if self.matches.is_empty() {
            "No results".into()
        } else {
            format!("{} of {}", self.current + 1, self.count())
        }
    }

    /// Hides the bar and drops the matches.
    pub fn close(&mut self) {
        self.open = false;
        self.matches.clear();
        self.current = 0;
        self.for_tab = None;
    }
}

impl AppState {
    /// Shows the find bar, re-running the query on the active tab.
    pub fn open_find(&mut self) {
        self.find.open = true;
        // Setting `focused` on a mounted field's TextEdit is how Rune moves keyboard focus into it.
        self.find.query.focused = true;
        self.sync_find();
    }

    /// Re-runs the query against the active tab's document. Called when the query, the active
    /// tab or its document changes. A tab that is not loaded has no matches.
    pub fn sync_find(&mut self) {
        if !self.find.open {
            return;
        }
        let Some(tab) = self.tabs.active() else {
            self.find.matches.clear();
            self.find.for_tab = None;
            return;
        };
        let (id, src) = (tab.id, match &tab.doc {
            DocState::Ready { src, .. } => Some(src.clone()),
            _ => None,
        });
        match src {
            Some(src) => self.find.run(strip_frontmatter(&src)),
            None => self.find.run(""),
        }
        self.find.for_tab = Some(id);
    }

    /// Steps to the next or previous match (`forward` false).
    pub fn find_step(&mut self, forward: bool) {
        if forward {
            self.find.next();
        } else {
            self.find.prev();
        }
    }
}

/// Scrolls the current match into view. Matches are keyed by their index (`Markdown::highlight_key`).
pub fn reveal_current(s: &AppState, tx: &mut Tx) {
    if s.find.count() > 0 {
        tx.scroll_into_view(Markdown::highlight_key(s.find.current));
    }
}

/// ⌘F and the titlebar button: opens the bar, or closes it when it is open.
pub fn toggle(s: &mut AppState, tx: &mut Tx) {
    if s.find.open {
        s.find.close();
    } else {
        s.open_find();
        reveal_current(s, tx);
    }
}

#[cfg(test)]
#[path = "find_tests.rs"]
mod tests;
