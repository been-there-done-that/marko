//! Lucide icons as `rune::Icon`s, drawn with `currentColor` strokes.
//!
//! The path data is taken from Lucide (https://lucide.dev), vendored under its ISC licence:
//!
//! ISC License
//!
//! Copyright (c) for portions of Lucide are held by Cole Bemis 2013-2022 as part of Feather
//! (MIT). All other copyright (c) for Lucide are held by Lucide Contributors 2022.
//!
//! Permission to use, copy, modify, and/or distribute this software for any purpose with or
//! without fee is hereby granted, provided that the above copyright notice and this permission
//! notice appear in all copies.
//!
//! THE SOFTWARE IS PROVIDED "AS IS" AND THE AUTHOR DISCLAIMS ALL WARRANTIES WITH REGARD TO THIS
//! SOFTWARE INCLUDING ALL IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS. IN NO EVENT SHALL
//! THE AUTHOR BE LIABLE FOR ANY SPECIAL, DIRECT, INDIRECT, OR CONSEQUENTIAL DAMAGES OR ANY
//! DAMAGES WHATSOEVER RESULTING FROM LOSS OF USE, DATA OR PROFITS, WHETHER IN AN ACTION OF
//! CONTRACT, NEGLIGENCE OR OTHER TORTIOUS ACTION, ARISING OUT OF OR IN CONNECTION WITH THE USE
//! OR PERFORMANCE OF THIS SOFTWARE.

use rune::Icon;

pub static PANEL_LEFT: Icon = Icon {
    name: "lucide-panel-left",
    view: 24.0,
    body: r#"<rect width="18" height="18" x="3" y="3" rx="2" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M9 3v18" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static FOLDER_OPEN: Icon = Icon {
    name: "lucide-folder-open",
    view: 24.0,
    body: r#"<path d="m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static FOLDER: Icon = Icon {
    name: "lucide-folder",
    view: 24.0,
    body: r#"<path d="M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static FILE_TEXT: Icon = Icon {
    name: "lucide-file-text",
    view: 24.0,
    body: r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M14 2v4a2 2 0 0 0 2 2h4" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M10 9H8" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M16 13H8" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M16 17H8" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static FILE: Icon = Icon {
    name: "lucide-file",
    view: 24.0,
    body: r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M14 2v4a2 2 0 0 0 2 2h4" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static FILE_PLUS: Icon = Icon {
    name: "lucide-file-plus",
    view: 24.0,
    body: r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M14 2v4a2 2 0 0 0 2 2h4" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M9 15h6" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M12 18v-6" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

// Not in ui-preview.html: standard Lucide `file-x` path data.
pub static FILE_X: Icon = Icon {
    name: "lucide-file-x",
    view: 24.0,
    body: r#"<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M14 2v4a2 2 0 0 0 2 2h4" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m14.5 12.5-5 5" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m9.5 12.5 5 5" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static SEARCH: Icon = Icon {
    name: "lucide-search",
    view: 24.0,
    body: r#"<circle cx="11" cy="11" r="8" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m21 21-4.3-4.3" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static ELLIPSIS: Icon = Icon {
    name: "lucide-ellipsis",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="1" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><circle cx="19" cy="12" r="1" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><circle cx="5" cy="12" r="1" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static X: Icon = Icon {
    name: "lucide-x",
    view: 24.0,
    body: r#"<path d="M18 6 6 18" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m6 6 12 12" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static EYE: Icon = Icon {
    name: "lucide-eye",
    view: 24.0,
    body: r#"<path d="M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><circle cx="12" cy="12" r="3" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static PENCIL: Icon = Icon {
    name: "lucide-pencil",
    view: 24.0,
    body: r#"<path d="M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m15 5 4 4" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static CHEVRON_RIGHT: Icon = Icon {
    name: "lucide-chevron-right",
    view: 24.0,
    body: r#"<path d="m9 18 6-6-6-6" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static CHEVRON_DOWN: Icon = Icon {
    name: "lucide-chevron-down",
    view: 24.0,
    body: r#"<path d="m6 9 6 6 6-6" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

// Not in ui-preview.html: standard Lucide `chevron-up` path data.
pub static CHEVRON_UP: Icon = Icon {
    name: "lucide-chevron-up",
    view: 24.0,
    body: r#"<path d="m18 15-6-6-6 6" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static COPY: Icon = Icon {
    name: "lucide-copy",
    view: 24.0,
    body: r#"<rect width="14" height="14" x="8" y="8" rx="2" ry="2" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static LINK: Icon = Icon {
    name: "lucide-link",
    view: 24.0,
    body: r#"<path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static ROTATE_CW: Icon = Icon {
    name: "lucide-rotate-cw",
    view: 24.0,
    body: r#"<path d="M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M21 3v5h-5" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static ZOOM_IN: Icon = Icon {
    name: "lucide-zoom-in",
    view: 24.0,
    body: r#"<circle cx="11" cy="11" r="8" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m21 21-4.3-4.3" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M11 8v6" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M8 11h6" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static ZOOM_OUT: Icon = Icon {
    name: "lucide-zoom-out",
    view: 24.0,
    body: r#"<circle cx="11" cy="11" r="8" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m21 21-4.3-4.3" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M8 11h6" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static MOVE_HORIZONTAL: Icon = Icon {
    name: "lucide-move-horizontal",
    view: 24.0,
    body: r#"<path d="m18 8 4 4-4 4" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M2 12h20" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m6 8-4 4 4 4" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static SUN: Icon = Icon {
    name: "lucide-sun",
    view: 24.0,
    body: r#"<circle cx="12" cy="12" r="4" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M12 2v2" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M12 20v2" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m4.93 4.93 1.41 1.41" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m17.66 17.66 1.41 1.41" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M2 12h2" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M20 12h2" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m6.34 17.66-1.41 1.41" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m19.07 4.93-1.41 1.41" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static TRASH_2: Icon = Icon {
    name: "lucide-trash-2",
    view: 24.0,
    body: r#"<path d="M3 6h18" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static DOWNLOAD: Icon = Icon {
    name: "lucide-download",
    view: 24.0,
    body: r#"<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="m7 10 5 5 5-5" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/><path d="M12 15V3" stroke="currentColor" fill="none" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

/// Every icon in this module, for tests and an icon sheet.
pub static ALL: &[&Icon] = &[
    &PANEL_LEFT,
    &FOLDER_OPEN,
    &FOLDER,
    &FILE_TEXT,
    &FILE,
    &FILE_PLUS,
    &FILE_X,
    &SEARCH,
    &ELLIPSIS,
    &X,
    &EYE,
    &PENCIL,
    &CHEVRON_RIGHT,
    &CHEVRON_DOWN,
    &CHEVRON_UP,
    &COPY,
    &LINK,
    &ROTATE_CW,
    &ZOOM_IN,
    &ZOOM_OUT,
    &MOVE_HORIZONTAL,
    &SUN,
    &TRASH_2,
    &DOWNLOAD,
];
