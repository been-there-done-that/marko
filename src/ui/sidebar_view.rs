//! The sidebar (§4.6): the "Open files" section, then one section per folder root, each a
//! virtualised list of its file tree.
//!
//! `rune_kit::Tree` takes plain `fn` getters for its model, so one `Tree` per root cannot pick
//! root `i` out of the state. Instead the sections are flattened into one virtual list, and the
//! rows are drawn with rune-kit's `TreeItem` and `TreeRow` over each root's `TreeModel`.

use std::borrow::Cow;
use std::rc::Rc;

use rune::{El, GrowthSpec, ScrollSpec, TextStyle, Tx, col, el, icon, row, text};
use rune_kit::{Kit, TreeItem, TreeRow, TreeStyle};

use crate::app::AppState;
use crate::icons;
use crate::tabs::Tab;
use crate::theme::{Look, self, Tokens};

/// One line of the sidebar list.
#[derive(Clone, Copy)]
enum Item {
    LooseHead,
    /// An open tab outside every root, by tab id.
    Loose(u64),
    RootHead(usize),
    /// A root's scanning, error or empty note.
    Status(usize, &'static str),
    /// A visible row of a root's tree: root index, then row index.
    Node(usize, usize),
}

/// The sidebar's rows for this frame, in order.
fn items(s: &AppState) -> Vec<Item> {
    let mut out = Vec::new();
    let loose: Vec<&Tab> = s.tabs.iter().filter(|t| s.sidebar.root_of(&t.path).is_none()).collect();
    if !loose.is_empty() {
        out.push(Item::LooseHead);
        out.extend(loose.iter().map(|t| Item::Loose(t.id)));
    }
    for (i, r) in s.sidebar.roots.iter().enumerate() {
        out.push(Item::RootHead(i));
        if r.scanning {
            out.push(Item::Status(i, "Scanning…"));
        } else if r.error.is_some() {
            out.push(Item::Status(i, "Can't read this folder"));
        } else if r.tree.is_empty() {
            out.push(Item::Status(i, "No Markdown files"));
        } else {
            out.extend((0..r.tree.visible_len()).map(|j| Item::Node(i, j)));
        }
    }
    out
}

/// The row's key: unique across the sidebar, and what tests and handlers find it by.
fn key(s: &AppState, item: Item) -> String {
    match item {
        Item::LooseHead => "side-loose".into(),
        Item::Loose(id) => format!("side-loose-{id}"),
        Item::RootHead(i) => format!("side-root-{i}"),
        Item::Status(i, _) => format!("side-status-{i}"),
        Item::Node(i, j) => {
            let id = s.sidebar.roots.get(i).and_then(|r| r.tree.visible_id(j)).unwrap_or_default();
            format!("side-{i}/{id}")
        }
    }
}

fn section_text(k: &Kit, t: &Tokens, label: &str) -> El {
    let style = TextStyle { size: 11.0, weight: 600.0, tracking: 0.44, ..k.ty.small };
    text(label.to_uppercase()).font(style).color(t.fg3)
}

fn head(s: &AppState, k: &Kit, t: &Tokens, item: Item) -> El {
    match item {
        Item::LooseHead => row().key("side-loose").h(theme::ROW_H).px(8.0).center_y().child(section_text(k, t, "Open files")),
        Item::RootHead(i) => {
            let name = s.sidebar.roots[i].name.clone();
            let remove = row()
                .key(format!("side-root-remove-{i}"))
                .w(22.0)
                .h(22.0)
                .center()
                .radius(4.0)
                .on_press(move |s: &mut AppState| s.sidebar.remove_root(i))
                .child(icon(&icons::X, 12.0).color(t.fg3));
            row().key(format!("side-root-{i}")).h(theme::ROW_H).px(8.0).center_y().justify_between()
                .child(section_text(k, t, &name))
                .child(remove)
        }
        _ => el(),
    }
}

fn status(k: &Kit, t: &Tokens, i: usize, label: &str) -> El {
    row().key(format!("side-status-{i}")).h(theme::ROW_H).px(24.0).center_y()
        .child(text(label).font(k.ty.small).color(t.fg3))
}

/// An open tab outside every root. Pressing it activates the tab.
fn loose_row(s: &AppState, k: &Kit, id: u64, style: &TreeStyle) -> El {
    let Some(tab) = s.tabs.iter().find(|tab| tab.id == id) else { return el() };
    let name = tab.path.file_name().map_or_else(|| tab.path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let key = format!("side-loose-{id}");
    let active = s.tabs.active().is_some_and(|a| a.id == id);
    let r = TreeRow::new(&key, 0).selected(active);
    TreeItem::new(name).icon(&icons::FILE_TEXT).build(k, &r, style).key(key).on_press(move |s: &mut AppState| s.tabs.activate(id))
}

/// One visible row of a root's tree. A folder opens or closes; a file opens as a tab.
fn node_row(s: &AppState, k: &Kit, style: &TreeStyle, i: usize, j: usize) -> El {
    let Some((node, r)) = s.sidebar.roots[i].tree.row(j) else { return el() };
    let id = r.id.to_owned();
    let expanded = r.expanded.is_some();
    let file = node.data.clone();
    let key = format!("side-{i}/{id}");
    let item = TreeItem::file(node, &r).build(k, &r, style);
    item.key(key).on_press_tx(move |s: &mut AppState, tx: &mut Tx| {
        let model = &mut s.sidebar.roots[i].tree;
        let plain = model.click(&id, tx.mods());
        if expanded && plain {
            model.toggle(&id);
        }
        if let Some(path) = &file {
            let now = s.now;
            s.open_paths(std::slice::from_ref(path), now);
        }
    })
}

/// The sidebar column at the left of the body. It is shown beside the document, or alone when
/// there are roots but no tab.
pub fn view(s: &AppState, look: &'static Look) -> El {
    let k: &'static Kit = &look.kit;
    let t: &'static Tokens = &look.tokens;
    let style = Rc::new(TreeStyle::from_kit(k));
    let list = Rc::new(items(s));
    let count = list.len();
    let row_h = style.row_height;
    let keys = list.clone();
    let rows = list;
    let style_rows = style.clone();
    let body = col().key("side-rows").w_full().growth(GrowthSpec::default()).virtual_rows(
        count,
        row_h,
        move |s: &AppState, i| Cow::Owned(key(s, keys[i])),
        move |s: &AppState, i| match rows[i] {
            item @ (Item::LooseHead | Item::RootHead(_)) => head(s, k, t, item),
            Item::Status(root, label) => status(k, t, root, label),
            Item::Loose(id) => loose_row(s, k, id, &style_rows),
            Item::Node(root, j) => node_row(s, k, &style_rows, root, j),
        },
    );
    col().key("side").w(theme::SIDE_W).h_full().min_h(0.0).fill(t.side)
        .scroll(ScrollSpec { stick_bottom: false })
        .child(body)
}
