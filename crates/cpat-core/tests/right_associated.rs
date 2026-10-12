//! v12's `RightAssociated.test.ts` parse behavior. The introspection tests
//! (`getTokens`, `getPatterns`, `find`, `getNextTokens`, parents) wait for
//! the intellisense milestone (M8); `isEqual` and `startedOnIndex` have no
//! counterpart (patterns keep no positions, DESIGN §2).

mod common;

use common::*;
use cpat_core::{Pattern, RightAssociated};

fn e() -> Pattern {
    reference("expression")
}

/// v12's `createTernaryExpression`.
fn ternary_expression() -> Pattern {
    let variables = choice(
        "variables",
        vec![lit("a", "a"), lit("b", "b"), lit("c", "c")],
    );
    let ternary = seq(
        "ternary",
        vec![
            e(),
            lit("question-mark", " ? "),
            e(),
            lit("colon", " : "),
            e(),
        ],
    );
    choice(
        "expression",
        vec![
            seq("add-expression", vec![e(), lit("add", " + "), e()]),
            right(ternary),
            variables,
        ],
    )
}

#[test]
fn it_has_no_name_and_wraps_its_child() {
    let marker = RightAssociated::new(lit("x", "x"));

    assert_eq!(Pattern::from(marker.clone()).name(), "");
    assert_eq!(marker.child().name(), "x");
}

#[test]
fn a_clone_keeps_the_id() {
    let marker: Pattern = right(lit("x", "x"));

    assert_eq!(marker.clone().id(), marker.id());
}

#[test]
fn renaming_it_changes_nothing() {
    // v12's `clone(name)` ignores the name: a reference to the marker
    // still produces the child's nodes, under the child's name.
    let marker = right(lit("x", "x")).renamed("other");

    assert_eq!(marker.name(), "");
}

#[test]
fn parse_delegates_to_the_child() {
    let mut marker = right(lit("greeting", "hello"));

    assert_eq!(parse_shape(&mut marker, "hello").0, "greeting:literal@0..5");
}

#[test]
fn test_delegates_to_the_child() {
    let mut marker = right(lit("greeting", "hello"));

    assert!(marker.test("hello"));
    assert!(!marker.test("world"));
}

#[test]
fn exec_delegates_to_the_child_on_failure() {
    // v12's `exec` is the child's, so the summary error names the child.
    let mut marker = right(lit("greeting", "hello"));

    assert_eq!(
        exec_outline(&mut marker, "world"),
        outlined("-", "greeting@0..5", "greeting@0..0")
    );
}

#[test]
fn a_ternary_is_right_associative_in_a_choice() {
    let mut expression = ternary_expression();

    assert_eq!(
        exec_shape(&mut expression, "a ? b : c"),
        "ternary:expression@0..9[a:literal@0..1 question-mark:literal@1..4 b:literal@4..5 \
         colon:literal@5..8 c:literal@8..9]"
    );
    // `a ? b : c ? a : b` groups as `a ? b : (c ? a : b)`.
    assert_eq!(
        exec_shape(&mut expression, "a ? b : c ? a : b"),
        "ternary:expression@0..17[a:literal@0..1 question-mark:literal@1..4 b:literal@4..5 \
         colon:literal@5..8 ternary:expression@8..17[c:literal@8..9 question-mark:literal@9..12 \
         a:literal@12..13 colon:literal@13..16 b:literal@16..17]]"
    );
    assert_eq!(
        exec_shape(&mut expression, "a ? b + c : a + b ? c : a"),
        "ternary:expression@0..25[a:literal@0..1 question-mark:literal@1..4 \
         add-expression:expression@4..9[b:literal@4..5 add:literal@5..8 c:literal@8..9] \
         colon:literal@9..12 ternary:expression@12..25[add-expression:expression@12..17[\
         a:literal@12..13 add:literal@13..16 b:literal@16..17] question-mark:literal@17..20 \
         c:literal@20..21 colon:literal@21..24 a:literal@24..25]]"
    );
}

#[test]
fn a_left_associated_operator_in_the_same_choice_still_nests_left() {
    let mut expression = ternary_expression();

    assert_eq!(
        exec_shape(&mut expression, "a + b"),
        "add-expression:expression@0..5[a:literal@0..1 add:literal@1..4 b:literal@4..5]"
    );
}

#[test]
fn it_marks_a_reference_alternative_too() {
    // `pow` is found through the reference at the first parse, and the
    // marker around the reference makes it right-associative.
    let mut root = context(
        "root",
        choice(
            "expr",
            vec![
                right(reference("pow")),
                seq(
                    "mul",
                    vec![reference("expr"), lit("*", "*"), reference("expr")],
                ),
                re("num", "[0-9]"),
            ],
        ),
        vec![seq(
            "pow",
            vec![reference("expr"), lit("^", "^"), reference("expr")],
        )],
    );

    assert_eq!(
        exec_shape(&mut root, "1^2^3"),
        "pow:expression@0..5[num:regex@0..1 ^:literal@1..2 pow:expression@2..5[num:regex@2..3 \
         ^:literal@3..4 num:regex@4..5]]"
    );
    assert_eq!(
        exec_shape(&mut root, "1*2^3*4"),
        "mul:expression@0..7[mul:expression@0..5[num:regex@0..1 *:literal@1..2 \
         pow:expression@2..5[num:regex@2..3 ^:literal@3..4 num:regex@4..5]] *:literal@5..6 \
         num:regex@6..7]"
    );
}
