mod common;

use common::*;
use cpat_core::{Cursor, Literal, NodeKind, PatternError};

#[test]
fn empty_value_is_rejected() {
    assert_eq!(
        Literal::new("empty", "").unwrap_err(),
        PatternError::EmptyLiteral {
            name: "empty".into()
        }
    );
}

#[test]
fn successful_parse() {
    let mut literal = lit("greeting", "Hello World!");
    let mut cursor = Cursor::new("Hello World!");

    let id = literal.parse(&mut cursor).unwrap();

    assert_eq!(cursor.pos(), 12, "the cursor sits just past the match");
    assert!(cursor.error().is_none());
    let leaf = cursor.leaf_match().unwrap();
    assert_eq!(
        (leaf.pattern.name, leaf.start, leaf.end),
        ("greeting".into(), 0, 12)
    );

    let tree = cursor.into_tree(id);
    let node = tree.root();
    assert_eq!(node.kind(), NodeKind::Literal);
    assert_eq!(node.name(), "greeting");
    assert_eq!((node.start(), node.end()), (0, 12));
    assert_eq!(node.value(), "Hello World!");
}

#[test]
fn failed_parse_points_at_the_first_difference() {
    let mut literal = lit("greeting", "Hello World!");
    let mut cursor = Cursor::new("Hello Saturn!");

    assert!(literal.parse(&mut cursor).is_none());

    let error = cursor.error().unwrap();
    assert_eq!((error.start, error.last), (0, 6));
    assert_eq!(error.pattern.name, "greeting");
    assert_eq!(
        cursor.pos(),
        0,
        "a failed pattern leaves the cursor where it started"
    );
}

#[test]
fn failed_parse_because_the_text_ended() {
    let mut literal = lit("greeting", "Hello World!");
    let mut cursor = Cursor::new("Hello World");

    assert!(literal.parse(&mut cursor).is_none());

    let error = cursor.error().unwrap();
    assert_eq!((error.start, error.last), (0, 10));
}

#[test]
fn clone_keeps_name_and_identity() {
    let literal = lit("greeting", "Hello World!");
    let clone = literal.clone();

    assert_eq!(clone.name(), "greeting");
    assert_eq!(clone.id(), literal.id());
}

#[test]
fn renamed_keeps_identity() {
    let literal = lit("a", "A");
    let renamed = literal.renamed("b");

    assert_eq!(renamed.name(), "b");
    assert_eq!(renamed.id(), literal.id());
}

#[test]
fn exec_and_test() {
    let mut literal = lit("a", "A");

    assert!(literal.exec("B").tree.is_none());
    assert!(!literal.test("B"));
    assert!(literal.test("A"));
}

#[test]
fn unicode() {
    for text in ["🔴", "|🔴|", "|🔴", "🔴|"] {
        let mut literal = lit("a", text);
        let mut cursor = Cursor::new(text);

        let id = literal.parse(&mut cursor).unwrap();

        assert_eq!(cursor.pos(), text.len(), "{text}");
        assert!(cursor.error().is_none(), "{text}");
        assert_eq!(cursor.into_tree(id).root().value(), text);
    }
}

#[test]
fn a_token_ending_inside_a_grapheme_covers_the_token_and_continues_after_the_grapheme() {
    // "e" + combining acute is one grapheme. The literal "e" matches its first
    // half; the node is exactly "e", and parsing resumes after the accent.
    let mut literal = lit("e", "e");
    let mut cursor = Cursor::new("e\u{301}x");

    let id = literal.parse(&mut cursor).unwrap();

    assert_eq!(cursor.pos(), 3);
    assert_eq!(cursor.into_tree(id).root().value(), "e");
}

#[test]
fn crlf_is_one_grapheme_so_cr_then_lf_literals_do_not_split_it() {
    // "\r" matches the first half of the CRLF grapheme and resumes after the
    // whole of it, so "\n" has nothing left to match, as in v12.
    let mut pattern = seq("s", vec![lit("cr", "\r"), lit("lf", "\n")]);
    let mut after_a = seq("s", vec![lit("a", "a"), lit("cr", "\r"), lit("lf", "\n")]);

    assert!(pattern.exec("\r\n").tree().is_none());
    assert!(after_a.exec("a\r\n").tree().is_none());
}
