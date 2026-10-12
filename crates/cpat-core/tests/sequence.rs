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

#[test]
fn a_lookahead_at_the_end_of_input_records_nothing_when_it_holds() {
    // v12 tries the lookahead's child against an empty probe, so the
    // child's failure never becomes the furthest error of the real parse.
    let mut at_end = seq("s", vec![lit("ab", "ab"), not("n", lit("c", ","))]);
    let result = at_end.exec("ab");
    assert!(result.is_match());
    assert!(result.furthest_error.is_none());
}

#[test]
fn a_lookahead_that_fails_at_the_end_of_input_is_its_own_error_past_the_end() {
    let mut at_end = seq(
        "at-end",
        vec![lit("a", "a"), not("not", opt("b?", lit("b", "b")))],
    );
    let mut cursor = Cursor::new("a");
    assert!(at_end.parse(&mut cursor).is_none());
    let error = cursor.error().unwrap();
    assert_eq!(
        (error.pattern.name.as_str(), error.start, error.last),
        ("not", 1, 1)
    );
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn a_last_child_that_moved_without_a_node_reports_from_where_it_stopped() {
    // `(a?)+` on "ab" consumes "a", then its next try matches nothing, so
    // it returns no node and no error with its cursor past the "a". With
    // nothing matched the sequence fails there, at 1, as in v12.
    let mut sequence = seq(
        "s",
        vec![rep("r", opt("o", lit("a", "a")), Default::default())],
    );
    let mut cursor = Cursor::new("ab");
    assert!(sequence.parse(&mut cursor).is_none());
    let error = cursor.error().unwrap();
    assert_eq!(
        (error.pattern.name.as_str(), error.start, error.last),
        ("s", 0, 1)
    );
    // A failed pattern leaves the cursor where it began (DESIGN §5).
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn running_out_of_text_reports_the_end_even_after_a_wide_grapheme() {
    // Deviation (DESIGN §11): v12 records `cursor.index + 1` here, one UTF-16
    // unit past the start of the last grapheme, which lands inside "e" +
    // combining acute (v12: `root@0..1`). Positions here stay on grapheme
    // boundaries, so the error ends at the end of the text. On ASCII the
    // two agree ("ab" gives `root@0..2` in both).
    let mut pattern = seq("root", vec![lit("e", "e\u{301}"), lit("e2", "x")]);

    let result = pattern.exec("e\u{301}");

    assert!(result.tree.is_none());
    assert_eq!(error_outline(result.furthest_error), "root@0..3");

    let mut ascii = seq("root", vec![lit("e", "ab"), lit("e2", "x")]);
    assert_eq!(error_outline(ascii.exec("ab").furthest_error), "root@0..2");
}
