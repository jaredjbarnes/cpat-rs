mod common;

use common::*;
use cpat_core::{Choice, Cursor, NodeKind, PatternError};

#[test]
fn no_alternatives_is_rejected() {
    assert!(matches!(
        Choice::new("bad", vec![]),
        Err(PatternError::NoChildren { .. })
    ));
}

#[test]
fn a_match_returns_the_alternatives_own_node() {
    let mut a = choice("a", vec![lit("a", "A")]);
    let mut cursor = Cursor::new("A");

    let id = a.parse(&mut cursor).unwrap();

    let tree = cursor.into_tree(id);
    assert_eq!(tree.root().kind(), NodeKind::Literal);
    assert_eq!(tree.root().name(), "a");
}

#[test]
fn no_match_is_an_error_at_the_start() {
    let mut a = choice("a", vec![lit("a", "A")]);
    let mut cursor = Cursor::new("B");

    assert!(a.parse(&mut cursor).is_none());

    let error = cursor.error().unwrap();
    assert_eq!((error.start, error.last), (0, 0));
    assert_eq!(error.pattern.name, "a");
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn alternatives_are_tried_in_order_at_the_same_position() {
    let mut a_or_b = choice("a-b", vec![lit("a", "A"), lit("b", "B")]);
    let mut cursor = Cursor::new("AB");

    let first = a_or_b.parse(&mut cursor).unwrap();
    assert_eq!(cursor.pos(), 1);
    let second = a_or_b.parse(&mut cursor).unwrap();
    assert_eq!(cursor.pos(), 2);

    let tree = cursor.into_tree(second);
    assert_eq!(tree.root().name(), "b");
    assert_eq!(tree.node(first).name(), "a");
}

#[test]
fn the_first_match_wins_even_when_a_later_one_is_longer() {
    let mut shadowed = choice("op", vec![lit("eq", "="), lit("eqeq", "==")]);
    let mut cursor = Cursor::new("==");

    let id = shadowed.parse(&mut cursor).unwrap();

    assert_eq!(cursor.into_tree(id).root().name(), "eq");
}

#[test]
fn exec_and_test() {
    let mut a = choice("a", vec![lit("a", "A")]);

    assert!(a.exec("B").tree.is_none());
    assert!(!a.test("B"));
    assert!(a.test("A"));
}

#[test]
fn a_failed_alternative_does_not_leak_its_error_into_a_later_success() {
    let mut a_or_ab = choice(
        "x",
        vec![
            seq("ab", vec![lit("a", "a"), lit("b", "b")]),
            lit("ac", "ac"),
        ],
    );
    let mut cursor = Cursor::new("ac");

    assert!(a_or_ab.parse(&mut cursor).is_some());
    assert!(!cursor.has_error());
}
