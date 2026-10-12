mod common;

use common::*;
use cpat_core::Cursor;

// ─── Optional ───────────────────────────────────────────────────────────

#[test]
fn optional_match() {
    let mut optional = opt("optional-text", lit("text", "Text"));

    let result = optional.exec("Text");

    assert_eq!(result.tree().unwrap().root().value(), "Text");
}

#[test]
fn optional_no_match_leaves_the_cursor_in_place() {
    let mut optional = opt("optional-text", lit("text", "Text"));
    let mut cursor = Cursor::new("Bad Text");

    assert!(optional.parse(&mut cursor).is_none());
    assert!(!cursor.has_error());
    assert_eq!(cursor.pos(), 0);

    assert!(optional.exec("Bad Text").tree.is_none());
}

#[test]
fn optional_test() {
    let mut optional = opt("optional-text", lit("text", "Text"));

    assert!(optional.test("Text"));
    assert!(!optional.test("Bad Text"));
}

#[test]
fn optional_clone_keeps_name() {
    let optional = opt("optional-a", lit("a", "A"));
    assert_eq!(optional.clone().name(), "optional-a");
}

// ─── Not ────────────────────────────────────────────────────────────────

#[test]
fn not_succeeds_when_its_child_fails() {
    let mut not_a = not("not-a", lit("a", "A"));
    let mut cursor = Cursor::new("B");

    assert!(not_a.parse(&mut cursor).is_none());
    assert!(!cursor.has_error());
    assert_eq!(cursor.pos(), 0, "a lookahead consumes nothing");
}

#[test]
fn not_fails_when_its_child_matches() {
    let mut not_a = not("not-a", lit("a", "A"));
    let mut cursor = Cursor::new("A");

    assert!(not_a.parse(&mut cursor).is_none());

    let error = cursor.error().unwrap();
    assert_eq!((error.start, error.last), (0, 0));
    assert_eq!(error.pattern.name, "not-a");
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn not_a_not() {
    let mut not_not_a = not("not-not-a", not("not-a", lit("a", "A")));
    let mut cursor = Cursor::new("A");

    assert!(not_not_a.parse(&mut cursor).is_none());
    assert!(!cursor.has_error());
}

#[test]
fn not_exec_and_test() {
    let mut not_a = not("not-a", lit("a", "A"));

    assert!(not_a.exec("A").tree.is_none());
    assert!(!not_a.test("A"));
}
