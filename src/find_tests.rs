use super::*;

const SRC: &str = "# Alpha\n\nalpha beta\n\n- Alpha item\n\nnothing here\n";

fn with_query(q: &str) -> Find {
    let mut f = Find::new();
    f.query = TextEdit::new(q);
    f
}

#[test]
fn empty_and_missing_query() {
    let mut f = with_query("");
    f.run(SRC);
    assert_eq!(f.count(), 0);
    assert_eq!(f.label(), "");
    f.next();
    f.prev();
    assert_eq!(f.current, 0);

    let mut f = with_query("zebra");
    f.run(SRC);
    assert_eq!(f.count(), 0);
    assert_eq!(f.label(), "No results");
    f.next();
    assert_eq!(f.current, 0);
}

#[test]
fn next_prev_wrap() {
    let mut f = with_query("alpha");
    f.run(SRC);
    let n = f.count();
    assert!(n >= 3, "expected three or more matches, got {n}");
    assert_eq!(f.label(), format!("1 of {n}"));
    f.prev();
    assert_eq!(f.current, n - 1, "previous from the first wraps to the last");
    f.next();
    assert_eq!(f.current, 0, "next from the last wraps to the first");
    f.next();
    assert_eq!(f.label(), format!("2 of {n}"));
}

#[test]
fn rerun_clamps_current() {
    let mut f = with_query("alpha");
    f.run(SRC);
    f.current = f.count() - 1;
    f.query = TextEdit::new("beta");
    f.run(SRC);
    assert_eq!(f.count(), 1);
    assert_eq!(f.current, 0);
    f.query = TextEdit::new("zebra");
    f.run(SRC);
    assert_eq!(f.current, 0);
    assert_eq!(f.label(), "No results");
}

#[test]
fn close_clears_matches() {
    let mut f = with_query("alpha");
    f.open = true;
    f.run(SRC);
    f.for_tab = Some(3);
    f.close();
    assert!(!f.open);
    assert_eq!(f.count(), 0);
    assert_eq!(f.for_tab, None);
}
