mod common;

use common::*;
use cpat_core::{Cursor, NodeKind, Pattern, PatternError, Sequence};

fn parse_tree(pattern: &mut Pattern, text: &str) -> Option<Vec<(String, usize, usize, String)>> {
    let mut cursor = Cursor::new(text);
    let id = pattern.parse(&mut cursor)?;
    let tree = cursor.into_tree(id);
    let root = tree.root();
    let mut nodes = vec![(
        root.name().to_string(),
        root.start(),
        root.end(),
        root.value().to_string(),
    )];
    nodes.extend(root.children().map(|c| {
        (
            c.name().to_string(),
            c.start(),
            c.end(),
            c.value().to_string(),
        )
    }));
    Some(nodes)
}

fn n(name: &str, start: usize, end: usize, value: &str) -> (String, usize, usize, String) {
    (name.to_string(), start, end, value.to_string())
}

#[test]
fn no_children_is_rejected() {
    assert!(matches!(
        Sequence::new("empty", vec![]),
        Err(PatternError::NoChildren { .. })
    ));
}

#[test]
fn one_pattern_match_successful() {
    let mut sequence = seq("sequence", vec![lit("a", "A")]);
    let mut cursor = Cursor::new("A");

    let id = sequence.parse(&mut cursor).unwrap();

    assert!(cursor.furthest_error().is_none());
    assert_eq!(cursor.pos(), 1);
    let tree = cursor.into_tree(id);
    assert_eq!(tree.root().kind(), NodeKind::Sequence);
    assert_eq!(tree.root().children().len(), 1);
}

#[test]
fn one_pattern_match_fails() {
    let mut sequence = seq("sequence", vec![lit("a", "A")]);
    let mut cursor = Cursor::new("V");

    assert!(sequence.parse(&mut cursor).is_none());

    let error = cursor.error().unwrap();
    assert_eq!((error.start, error.last), (0, 0));
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn two_pattern_match_successful() {
    let mut sequence = seq("sequence", vec![lit("a", "A"), lit("b", "B")]);

    assert_eq!(
        parse_tree(&mut sequence, "AB").unwrap(),
        vec![
            n("sequence", 0, 2, "AB"),
            n("a", 0, 1, "A"),
            n("b", 1, 2, "B")
        ]
    );
}

#[test]
fn two_pattern_match_fails() {
    let mut sequence = seq("sequence", vec![lit("a", "A"), lit("b", "B")]);
    let mut cursor = Cursor::new("AC");

    assert!(sequence.parse(&mut cursor).is_none());

    let error = cursor.error().unwrap();
    assert_eq!((error.start, error.last), (1, 1));
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn a_failure_inside_an_optional_sequence_is_no_error() {
    let mut optional = opt("optional-sequence", seq("sequence", vec![lit("a", "A")]));
    let mut cursor = Cursor::new("V");

    assert!(optional.parse(&mut cursor).is_none());
    assert!(cursor.error().is_none());
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn trailing_optional_child_patterns() {
    let mut sequence = seq("sequence", vec![lit("a", "A"), opt("b", lit("b", "B"))]);
    let mut cursor = Cursor::new("AD");

    let id = sequence.parse(&mut cursor).unwrap();

    assert!(cursor.error().is_none());
    assert_eq!(cursor.pos(), 1);
    assert_eq!(cursor.into_tree(id).root().value(), "A");
}

#[test]
fn trailing_optional_child_patterns_with_no_more_text() {
    let mut sequence = seq("sequence", vec![lit("a", "A"), opt("b", lit("b", "B"))]);

    assert_eq!(
        parse_tree(&mut sequence, "A").unwrap(),
        vec![n("sequence", 0, 1, "A"), n("a", 0, 1, "A")]
    );
}

#[test]
fn text_ending_before_a_required_child_is_an_error_past_the_end() {
    let mut sequence = seq("sequence", vec![lit("a", "A"), lit("b", "B")]);
    let mut cursor = Cursor::new("A");

    assert!(sequence.parse(&mut cursor).is_none());

    let error = cursor.error().unwrap();
    assert_eq!(
        (error.start, error.last),
        (0, 1),
        "`last` is the end itself: the input stopped short"
    );
    assert_eq!(error.pattern.name, "sequence");
}

#[test]
fn incomplete_parse_inside_an_optional_is_no_error() {
    let mut optional = opt(
        "optional-sequence",
        seq("sequence", vec![lit("a", "A"), lit("b", "B")]),
    );
    let mut cursor = Cursor::new("A");

    assert!(optional.parse(&mut cursor).is_none());
    assert!(cursor.error().is_none());
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn optional_child_pattern_with_more_patterns() {
    let mut sequence = seq(
        "sequence",
        vec![opt("a", lit("a", "A")), lit("b", "B"), lit("c", "C")],
    );

    assert_eq!(
        parse_tree(&mut sequence, "BC").unwrap(),
        vec![
            n("sequence", 0, 2, "BC"),
            n("b", 0, 1, "B"),
            n("c", 1, 2, "C")
        ]
    );
}

#[test]
fn no_matches_on_optional_children_is_no_node_and_no_error() {
    let mut sequence = seq(
        "sequence",
        vec![opt("a", lit("a", "A")), opt("b", lit("b", "B"))],
    );
    let mut cursor = Cursor::new("XYZ");

    assert!(sequence.parse(&mut cursor).is_none());
    assert!(!cursor.has_error());
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn exec_and_test() {
    let mut sequence = seq("sequence", vec![lit("a", "A")]);

    let result = sequence.exec("A");
    assert_eq!(result.tree().unwrap().root().value(), "A");
    assert!(sequence.test("A"));
    assert!(!sequence.test("B"));
}

#[test]
fn a_partial_match_is_not_a_match() {
    let mut sequence = seq("sequence", vec![lit("a", "A")]);

    let result = sequence.exec("AB");

    assert!(result.tree.is_none());
    let error = result.error.unwrap();
    assert_eq!((error.start, error.last), (1, 2));
}

#[test]
fn a_negative_lookahead_holds_at_the_end_of_input() {
    // Nothing follows "a", so `!"b"` is true. (The TS library fails this; v12
    // fixes it there too.)
    let mut at_end = seq("at-end", vec![lit("a", "a"), not("not-b", lit("b", "b"))]);
    assert!(at_end.test("a"));

    let mut in_middle = seq(
        "in-middle",
        vec![lit("a", "a"), not("not-b", lit("b", "b")), lit("c", "c")],
    );
    assert!(in_middle.test("ac"));
    assert!(!in_middle.test("ab"));
}

#[test]
fn a_lookahead_whose_child_matches_empty_fails_at_the_end_of_input() {
    // `b?` matches (as nothing) anywhere, including at the end, so `!(b?)`
    // never holds.
    let mut at_end = seq(
        "at-end",
        vec![lit("a", "a"), not("not", opt("b?", lit("b", "b")))],
    );
    assert!(!at_end.test("a"));
}
