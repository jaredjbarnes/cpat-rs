mod common;

use common::*;

#[test]
fn empty_text_is_no_match_and_no_error() {
    let mut a = lit("a", "a");

    let result = a.exec("");

    assert!(result.tree.is_none());
    assert!(result.error.is_none());
}

// BasePattern.test.ts "exec keeps the real furthest error": the first two
// tests are this one and the next. The third ("the error message is
// unchanged") waits for error messages.
#[test]
fn a_failed_exec_keeps_the_deepest_real_failure_as_the_furthest_error() {
    // "Dot" fails against "Doe" at offset 7. The summary error ("didn't match
    // the text", ending at the text length) must not displace it. (The TS
    // library loses it; v12 fixes that too.)
    let mut full_name = seq(
        "full-name",
        vec![
            lit("first", "John"),
            re("space", r"\s+"),
            lit("last", "Doe"),
        ],
    );

    let result = full_name.exec("John Dot");

    let error = result.error.unwrap();
    assert_eq!(
        (error.start, error.last, error.pattern.name.as_str()),
        (0, 8, "full-name")
    );
    let furthest = result.furthest_error.unwrap();
    assert_eq!(
        (
            furthest.start,
            furthest.last,
            furthest.pattern.name.as_str()
        ),
        (5, 7, "last")
    );
}

#[test]
fn a_partial_match_with_no_failures_reports_the_leftover_text() {
    let mut a = lit("a", "a");

    let result = a.exec("ab");

    let furthest = result.furthest_error.unwrap();
    assert_eq!((furthest.start, furthest.last), (1, 2));
}

#[test]
fn a_direct_parse_finishes_into_a_result_as_exec_returns_one() {
    // `Cursor::finish` is how a host driving `parse` itself (v12's
    // `pattern.parse(cursor)`) gets the tree and the report: here from a
    // position past the start, and kept although it doesn't span the text.
    let mut world = lit("world", "World");
    let mut cursor = cpat_core::Cursor::new("Hello World!");
    cursor.set_pos(6);
    let root = world.parse(&mut cursor);
    let result = cursor.finish(root);

    let tree = result.tree().expect("the literal matched");
    assert_eq!((tree.root().start(), tree.root().end()), (6, 11));
    assert_eq!(result.leaf_match.expect("a leaf match").end, 11);

    let mut cursor = cpat_core::Cursor::new("Hello World!");
    let root = world.parse(&mut cursor);
    let result = cursor.finish(root);
    assert!(result.tree().is_none());
    assert_eq!(result.error.expect("an error").pattern.name, "world");
}
