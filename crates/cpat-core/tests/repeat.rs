//! Ports of the parse-behavior tests in clarity-pattern-parser v12's
//! `Repeat.test.ts`, `InfiniteRepeat.test.ts` and `FiniteRepeat.test.ts`,
//! plus tests that pin v12's edge behavior.
//!
//! Positions are half-open: a TS inclusive `lastIndex` L is `end = L + 1`
//! here, and a TS `cursor.index` of L after a success is `pos = L + 1`.
//!
//! The introspection tests of all three files (getTokens, getTokensAfter,
//! getNextTokens, getPatterns, getPatternsAfter, getNextPatterns, find,
//! clone, isEqual) are in `tests/introspect_repeat.rs`.
//!
//! Not ported: FiniteRepeat.test.ts "startedOnIndex reflects last parse
//! position". Rust patterns keep no parse position for others to read; the
//! cursor's ancestor stack takes that role (DESIGN §2).

mod common;

use common::*;
use cpat_core::{
    Cursor, Custom, CustomParse, Meta, NodeId, NodeKind, Pattern, Repeat, RepeatOptions,
};

/// One node, flattened: kind, name, half-open range, value.
type N = (NodeKind, String, usize, usize, String);

fn n(kind: NodeKind, name: &str, start: usize, end: usize, value: &str) -> N {
    (kind, name.to_string(), start, end, value.to_string())
}

fn digit(start: usize, value: &str) -> N {
    n(NodeKind::Regex, "digit", start, start + value.len(), value)
}

fn number(start: usize, value: &str) -> N {
    n(NodeKind::Regex, "number", start, start + value.len(), value)
}

/// What `parse` did: the root and its children, the cursor position, and
/// the current error as `(pattern, start, last)`.
struct Outcome {
    nodes: Option<Vec<N>>,
    pos: usize,
    error: Option<(&'static str, usize, usize)>,
}

fn parse(pattern: &mut Pattern, text: &str) -> Outcome {
    let mut cursor = Cursor::new(text);
    let id = pattern.parse(&mut cursor);
    let pos = cursor.pos();
    let error = cursor
        .error()
        .map(|e| (e.pattern.name.as_str(), e.start, e.last));
    let nodes = id.map(|id| {
        let tree = cursor.into_tree(id);
        let root = tree.root();
        let flat = |node: cpat_core::Node<'_, '_>| {
            n(
                node.kind(),
                node.name(),
                node.start(),
                node.end(),
                node.value(),
            )
        };
        let mut nodes = vec![flat(root)];
        nodes.extend(root.children().map(flat));
        nodes
    });
    Outcome { nodes, pos, error }
}

fn digits() -> Pattern {
    re("digit", r"\d")
}

fn numbers_child() -> Pattern {
    re("number", r"\d")
}

fn bounded(min: usize, max: usize) -> RepeatOptions {
    RepeatOptions {
        min,
        max: Some(max),
        ..RepeatOptions::default()
    }
}

fn at_least(min: usize) -> RepeatOptions {
    RepeatOptions {
        min,
        ..RepeatOptions::default()
    }
}

// ─── Repeat.test.ts ─────────────────────────────────────────────────────

#[test]
fn finite_repeat_without_min() {
    // Repeat.test.ts "Finite Repeat Without Min"
    let mut repeat = rep(
        "numbers",
        numbers_child(),
        RepeatOptions {
            max: Some(2),
            ..RepeatOptions::default()
        },
    );

    let failed = parse(&mut repeat, "f");
    assert!(failed.nodes.is_none());
    assert!(failed.error.is_some());

    let one = parse(&mut repeat, "1");
    assert_eq!(
        one.nodes.unwrap(),
        vec![
            n(NodeKind::FiniteRepeat, "numbers", 0, 1, "1"),
            number(0, "1")
        ]
    );
    assert!(one.error.is_none());

    let expected_two = vec![
        n(NodeKind::FiniteRepeat, "numbers", 0, 2, "12"),
        number(0, "1"),
        number(1, "2"),
    ];
    let two = parse(&mut repeat, "12");
    assert_eq!(two.nodes.unwrap(), expected_two);
    assert!(two.error.is_none());

    let three = parse(&mut repeat, "123");
    assert_eq!(three.nodes.unwrap(), expected_two);
    assert!(three.error.is_none());
    assert_eq!(three.pos, 2, "TS cursor.index 1");
}

#[test]
fn finite_repeat_with_min() {
    // Repeat.test.ts "Finite Repeat With Min"
    let mut repeat = rep("numbers", numbers_child(), bounded(2, 2));

    let failed = parse(&mut repeat, "f");
    assert!(failed.nodes.is_none());
    assert!(failed.error.is_some());

    let short = parse(&mut repeat, "1");
    assert!(short.nodes.is_none());
    assert!(short.error.is_some());

    let expected = vec![
        n(NodeKind::FiniteRepeat, "numbers", 0, 2, "12"),
        number(0, "1"),
        number(1, "2"),
    ];
    let two = parse(&mut repeat, "12");
    assert_eq!(two.nodes.unwrap(), expected);
    assert!(two.error.is_none());

    let three = parse(&mut repeat, "123");
    assert_eq!(three.nodes.unwrap(), expected);
    assert!(three.error.is_none());
    assert_eq!(three.pos, 2, "TS cursor.index 1");
}

#[test]
fn repeat_properties() {
    // Repeat.test.ts "Repeat Properties": no max means the unbounded kind.
    let mut repeat = rep("numbers", lit("number", "1"), RepeatOptions::default());

    assert_eq!(repeat.name(), "numbers");
    let nodes = parse(&mut repeat, "1").nodes.unwrap();
    assert_eq!(nodes[0].0, NodeKind::InfiniteRepeat);
}

#[test]
fn repeat_accessors_return_configured_values() {
    // Repeat.test.ts "Repeat accessors return configured values"
    let repeat = Repeat::new("numbers", lit("number", "1"), bounded(2, 5));

    assert_eq!(repeat.min(), 2);
    assert_eq!(repeat.max(), Some(5));
    assert_eq!(repeat.child().name(), "number");
    assert!(repeat.divider().is_none());
    assert!(!repeat.trim_divider());
}

#[test]
fn repeat_default_min_and_max() {
    // Repeat.test.ts "Repeat default min and max"
    let repeat = Repeat::new("numbers", lit("number", "1"), RepeatOptions::default());

    assert_eq!(repeat.min(), 1);
    assert_eq!(repeat.max(), None);
}

#[test]
fn repeat_test() {
    // Repeat.test.ts "test"
    let mut repeat = rep("numbers", lit("number", "1"), RepeatOptions::default());

    assert!(repeat.test("1"));
    assert!(!repeat.test("g"));
}

#[test]
fn repeat_clone() {
    // Repeat.test.ts "Repeat Clone"
    let repeat = rep("numbers", lit("number", "1"), RepeatOptions::default());

    let clone = repeat.clone();
    assert_eq!(clone.name(), "numbers");
    assert_eq!(clone.id(), repeat.id());

    let mut renamed = repeat.clone_named("new-name");
    assert_eq!(renamed.name(), "new-name");
    let nodes = parse(&mut renamed, "11").nodes.unwrap();
    assert_eq!(
        nodes[0],
        n(NodeKind::InfiniteRepeat, "new-name", 0, 2, "11")
    );
}

// ─── InfiniteRepeat.test.ts ─────────────────────────────────────────────

#[test]
fn infinite_successful_parse() {
    // InfiniteRepeat.test.ts "Successful Parse"
    let mut integer = rep("number", digits(), RepeatOptions::default());
    let outcome = parse(&mut integer, "337");

    assert_eq!(
        outcome.nodes.unwrap(),
        vec![
            n(NodeKind::InfiniteRepeat, "number", 0, 3, "337"),
            digit(0, "3"),
            digit(1, "3"),
            digit(2, "7"),
        ]
    );
    assert!(outcome.error.is_none());
}

#[test]
fn infinite_bounds() {
    // InfiniteRepeat.test.ts "Bounds"
    let mut integer = rep("number", digits(), at_least(2));

    let short = parse(&mut integer, "3");
    assert!(short.nodes.is_none());
    assert!(short.error.is_some());

    let enough = parse(&mut integer, "33");
    assert_eq!(
        enough.nodes.unwrap(),
        vec![
            n(NodeKind::InfiniteRepeat, "number", 0, 2, "33"),
            digit(0, "3"),
            digit(1, "3"),
        ]
    );
    assert!(enough.error.is_none());
}

#[test]
fn infinite_failed_parse() {
    // InfiniteRepeat.test.ts "Failed Parse"
    let mut integer = rep("number", digits(), RepeatOptions::default());
    let outcome = parse(&mut integer, "John");

    assert!(outcome.nodes.is_none());
    assert!(outcome.error.is_some());
}

fn digits_and_dividers(text: &str) -> Vec<N> {
    // Alternating "d,d,d" nodes, as the divider tests expect.
    text.char_indices()
        .map(|(i, c)| {
            let value = c.to_string();
            if c == ',' {
                n(NodeKind::Literal, "divider", i, i + 1, &value)
            } else {
                digit(i, &value)
            }
        })
        .collect()
}

#[test]
fn infinite_successful_parse_with_divider() {
    // InfiniteRepeat.test.ts "Successful Parse With Divider"
    let mut integer = rep("number", digits(), divided(lit("divider", ",")));
    let outcome = parse(&mut integer, "3,3,7");

    let mut expected = vec![n(NodeKind::InfiniteRepeat, "number", 0, 5, "3,3,7")];
    expected.extend(digits_and_dividers("3,3,7"));
    assert_eq!(outcome.nodes.unwrap(), expected);
    assert!(outcome.error.is_none());
}

fn trimmed_divider() -> RepeatOptions {
    RepeatOptions {
        trim_divider: true,
        ..divided(lit("divider", ","))
    }
}

#[test]
fn infinite_successful_parse_text_ends_with_divider() {
    // InfiniteRepeat.test.ts "Successful Parse Text Ends With Divider"
    let mut integer = rep("number", digits(), trimmed_divider());
    let outcome = parse(&mut integer, "3,3,7,");

    let mut expected = vec![n(NodeKind::InfiniteRepeat, "number", 0, 5, "3,3,7")];
    expected.extend(digits_and_dividers("3,3,7"));
    assert_eq!(outcome.nodes.unwrap(), expected);
    assert!(outcome.error.is_none());
    assert_eq!(outcome.pos, 5, "the trimmed divider is given back");
}

#[test]
fn infinite_successful_parse_trailing_comma() {
    // InfiniteRepeat.test.ts "Successful Parse Trailing Comma"
    let mut integer = rep("number", digits(), trimmed_divider());
    let outcome = parse(&mut integer, "3,3,7,t");

    let mut expected = vec![n(NodeKind::InfiniteRepeat, "number", 0, 5, "3,3,7")];
    expected.extend(digits_and_dividers("3,3,7"));
    assert_eq!(outcome.nodes.unwrap(), expected);
    assert!(outcome.error.is_none());
    assert_eq!(
        outcome.pos, 5,
        "the cursor is back before the trimmed divider"
    );
}

#[test]
fn infinite_failed_optional() {
    // InfiniteRepeat.test.ts "Failed (Optional)": a zero-width child gives
    // no node and no error.
    let mut integer = rep("number", opt("optional-digit", digits()), at_least(0));
    let outcome = parse(&mut integer, "John");

    assert!(outcome.nodes.is_none());
    assert!(outcome.error.is_none());
}

#[test]
fn infinite_properties() {
    // InfiniteRepeat.test.ts "Properties"
    let integer = Repeat::new("integer", digits(), RepeatOptions::default());

    assert_eq!(integer.min(), 1);
    assert_eq!(integer.child().name(), "digit");
    assert_eq!(Pattern::from(integer).name(), "integer");
}

#[test]
fn infinite_exec() {
    // InfiniteRepeat.test.ts "Exec"
    let mut integer = rep("integer", digits(), RepeatOptions::default());
    assert!(integer.exec("B").tree().is_none());
}

#[test]
fn infinite_test_with_match() {
    // InfiniteRepeat.test.ts "Test With Match"
    let mut integer = rep("integer", digits(), RepeatOptions::default());
    assert!(integer.test("1"));
}

#[test]
fn infinite_test_with_no_match() {
    // InfiniteRepeat.test.ts "Test With No Match"
    let mut integer = rep("integer", digits(), RepeatOptions::default());
    assert!(!integer.test("b"));
}

#[test]
fn infinite_find_pattern() {
    // InfiniteRepeat.test.ts "Find Pattern" (the child is reachable).
    let integer = Repeat::new("integer", digits(), RepeatOptions::default());
    assert_eq!(integer.child().name(), "digit");
}

#[test]
fn infinite_clone_with_custom_overrides() {
    // InfiniteRepeat.test.ts "Clone With Custom Overrides"
    let numbers = Repeat::new(
        "numbers",
        numbers_child(),
        RepeatOptions {
            min: 3,
            divider: Some(lit("divider", "divider")),
            trim_divider: true,
            max: None,
        },
    );
    let clone = numbers.clone();

    assert_eq!(clone.min(), 3);
    assert_eq!(clone.max(), None);
    assert_eq!(clone.divider().unwrap().name(), "divider");
    assert!(clone.trim_divider());
    assert_eq!(clone.child().name(), "number");
}

#[test]
fn infinite_no_results_min_is_0() {
    // InfiniteRepeat.test.ts "No Results, min is 0"
    let mut numbers = rep(
        "numbers",
        numbers_child(),
        RepeatOptions {
            min: 0,
            ..divided(lit(",", ","))
        },
    );
    let result = numbers.exec("j");

    assert!(result.tree().is_none());
    assert!(result.error.is_some());

    let outcome = parse(&mut numbers, "j");
    assert_eq!(outcome.pos, 0);
}

fn comma_space() -> Pattern {
    seq("comma", vec![lit(",", ","), lit("space", " ")])
}

#[test]
fn infinite_trim_trailing_complex_delimiter_pattern() {
    // InfiniteRepeat.test.ts "Trim Trailing Complex Delimiter Pattern"
    let mut numbers = rep(
        "numbers",
        numbers_child(),
        RepeatOptions {
            trim_divider: true,
            ..divided(comma_space())
        },
    );
    let outcome = parse(&mut numbers, "1, 2,");

    assert_eq!(outcome.nodes.unwrap()[0].4, "1, 2");
}

#[test]
fn infinite_divider_repeat_where_neither_pattern_nor_divider_matches() {
    // InfiniteRepeat.test.ts "Divider repeat where neither pattern nor
    // divider matches on first try"
    let mut repeat = rep("numbers", digits(), divided(lit("divider", ",")));
    let outcome = parse(&mut repeat, "abc");

    assert!(outcome.nodes.is_none());
    assert!(outcome.error.is_some());
}

#[test]
fn infinite_trim_divider_leaves_the_digit() {
    // InfiniteRepeat.test.ts "Trim divider results in empty nodes returns
    // null" (despite its name, it checks that "1," trims to "1").
    let mut repeat = rep("numbers", digits(), trimmed_divider());
    let outcome = parse(&mut repeat, "1,x");

    assert_eq!(outcome.nodes.unwrap()[0].4, "1");
}

// ─── FiniteRepeat.test.ts ───────────────────────────────────────────────

fn finite(text: &str) -> Vec<N> {
    let mut nodes = vec![n(NodeKind::FiniteRepeat, "numbers", 0, text.len(), text)];
    nodes.extend(text.char_indices().map(|(i, c)| {
        let value = c.to_string();
        if c == ',' {
            n(NodeKind::Literal, "comma", i, i + 1, &value)
        } else {
            number(i, &value)
        }
    }));
    nodes
}

#[test]
fn finite_bounds_without_divider() {
    // FiniteRepeat.test.ts "Bounds Without Divider"
    let mut numbers = rep("numbers", numbers_child(), bounded(2, 3));

    let short = parse(&mut numbers, "1");
    assert!(short.nodes.is_none());
    assert!(short.error.is_some());

    let two = parse(&mut numbers, "12");
    assert_eq!(two.nodes.unwrap(), finite("12"));
    assert!(two.error.is_none());

    let three = parse(&mut numbers, "123");
    assert_eq!(three.nodes.unwrap(), finite("123"));
    assert!(three.error.is_none());

    let four = parse(&mut numbers, "1234");
    assert_eq!(four.nodes.unwrap(), finite("123"));
    assert!(four.error.is_none());
    assert_eq!(four.pos, 3, "TS cursor.index 2");

    let stops = parse(&mut numbers, "12f");
    assert_eq!(stops.nodes.unwrap(), finite("12"));
    assert!(stops.error.is_none());
}

#[test]
fn finite_bounds_are_equal_without_divider() {
    // FiniteRepeat.test.ts "Bounds Are Equal Without Divider"
    let mut numbers = rep("numbers", numbers_child(), bounded(3, 3));

    for short in ["1", "12"] {
        let outcome = parse(&mut numbers, short);
        assert!(outcome.nodes.is_none());
        assert!(outcome.error.is_some());
    }

    let three = parse(&mut numbers, "123");
    assert_eq!(three.nodes.unwrap(), finite("123"));
    assert!(three.error.is_none());

    let four = parse(&mut numbers, "1234");
    assert_eq!(four.nodes.unwrap(), finite("123"));
    assert!(four.error.is_none());
    assert_eq!(four.pos, 3, "TS cursor.index 2");
}

fn bounded_comma(min: usize, max: usize) -> RepeatOptions {
    RepeatOptions {
        min,
        max: Some(max),
        divider: Some(lit("comma", ",")),
        trim_divider: true,
    }
}

#[test]
fn finite_bounds_with_divider() {
    // FiniteRepeat.test.ts "Bounds With Divider"
    let mut numbers = rep("numbers", numbers_child(), bounded_comma(2, 3));

    let short = parse(&mut numbers, "1,");
    assert!(short.nodes.is_none());
    assert!(short.error.is_some());

    let two = parse(&mut numbers, "1,2");
    assert_eq!(two.nodes.unwrap(), finite("1,2"));
    assert!(two.error.is_none());

    let trailing = parse(&mut numbers, "1,2,3,");
    assert_eq!(trailing.nodes.unwrap(), finite("1,2,3"));
    assert!(trailing.error.is_none());

    let more = parse(&mut numbers, "1,2,3,4");
    assert_eq!(more.nodes.unwrap(), finite("1,2,3"));
    assert!(more.error.is_none());
}

#[test]
fn finite_optional_repeating_pattern() {
    // FiniteRepeat.test.ts "Optional Repeating Pattern": a zero-width child
    // slot still counts, and its divider still runs.
    let mut integer = rep(
        "number",
        opt("optional-digit", re("digit", r"\d+")),
        RepeatOptions {
            max: Some(4),
            ..divided(re("divider", r"\s"))
        },
    );
    let outcome = parse(&mut integer, "\n3\n\n\n");

    let divider = |i: usize| n(NodeKind::Regex, "divider", i, i + 1, "\n");
    assert_eq!(
        outcome.nodes.unwrap(),
        vec![
            n(NodeKind::FiniteRepeat, "number", 0, 5, "\n3\n\n\n"),
            divider(0),
            digit(1, "3"),
            divider(2),
            divider(3),
            divider(4),
        ]
    );
    assert!(outcome.error.is_none());
}

#[test]
fn finite_bounds_are_equal_with_divider() {
    // FiniteRepeat.test.ts "Bounds Are Equal With Divider"
    let mut numbers = rep("numbers", numbers_child(), bounded_comma(3, 3));

    for short in ["1,", "1,2"] {
        let outcome = parse(&mut numbers, short);
        assert!(outcome.nodes.is_none());
        assert!(outcome.error.is_some());
    }

    let trailing = parse(&mut numbers, "1,2,3,");
    assert_eq!(trailing.nodes.unwrap(), finite("1,2,3"));
    assert!(trailing.error.is_none());

    let more = parse(&mut numbers, "1,2,3,4");
    assert_eq!(more.nodes.unwrap(), finite("1,2,3"));
    assert!(more.error.is_none());
}

fn up_to_three() -> RepeatOptions {
    RepeatOptions {
        max: Some(3),
        ..RepeatOptions::default()
    }
}

#[test]
fn finite_test() {
    // FiniteRepeat.test.ts "Test"
    let mut numbers = rep("numbers", numbers_child(), up_to_three());
    assert!(numbers.test("1"));
}

#[test]
fn finite_exec() {
    // FiniteRepeat.test.ts "Exec"
    let mut numbers = rep("numbers", numbers_child(), up_to_three());
    let result = numbers.exec("1");

    assert!(result.tree().is_some());
    assert!(result.error.is_none());
}

#[test]
fn finite_fail() {
    // FiniteRepeat.test.ts "Fail"
    let mut numbers = rep("numbers", numbers_child(), up_to_three());
    let result = numbers.exec("f");

    assert!(result.tree().is_none());
    assert!(result.error.is_some());
}

#[test]
fn finite_optional() {
    // FiniteRepeat.test.ts "Optional"
    let mut numbers = rep(
        "numbers",
        opt("optional-number", numbers_child()),
        bounded(0, 3),
    );
    let result = numbers.exec("f");

    assert!(result.tree().is_none());
    assert!(result.error.is_some());
}

#[test]
fn finite_multiple_matches_but_not_the_whole_text() {
    // FiniteRepeat.test.ts "Optional With Multiple Matches But Still Below Min"
    let mut numbers = rep("numbers", numbers_child(), bounded(0, 3));
    let result = numbers.exec("12f");

    assert!(result.tree().is_none());
    assert!(result.error.is_some());
}

#[test]
fn finite_properties() {
    // FiniteRepeat.test.ts "Properties"
    let numbers = Repeat::new("numbers", numbers_child(), bounded(1, 3));

    assert_eq!(numbers.min(), 1);
    assert_eq!(numbers.max(), Some(3));
    let mut pattern = Pattern::from(numbers);
    assert_eq!(pattern.name(), "numbers");
    assert_eq!(
        parse(&mut pattern, "1").nodes.unwrap()[0].0,
        NodeKind::FiniteRepeat
    );
}

#[test]
fn finite_clone() {
    // FiniteRepeat.test.ts "Clone" and "Clone With Custom Overrides"
    let numbers = Repeat::new("numbers", numbers_child(), bounded(0, 3));
    let clone = numbers.clone();

    assert_eq!(clone.min(), numbers.min());
    assert_eq!(clone.max(), numbers.max());

    let renamed = Pattern::from(numbers).clone_named("cloned-numbers");
    assert_eq!(renamed.name(), "cloned-numbers");
}

#[test]
fn finite_find_pattern() {
    // FiniteRepeat.test.ts "Find Pattern" (the divider is reachable).
    let numbers = Repeat::new(
        "numbers",
        lit("one", "1"),
        RepeatOptions {
            min: 0,
            max: Some(2),
            ..divided(lit("comma", ","))
        },
    );
    assert_eq!(numbers.divider().unwrap().name(), "comma");
}

#[test]
fn finite_trim_trailing_complex_delimiter_pattern() {
    // FiniteRepeat.test.ts "Trim Trailing Complex Delimiter Pattern"
    let mut numbers = rep(
        "numbers",
        numbers_child(),
        RepeatOptions {
            max: Some(3),
            trim_divider: true,
            ..divided(comma_space())
        },
    );
    let outcome = parse(&mut numbers, "1, 2,");

    assert_eq!(outcome.nodes.unwrap()[0].4, "1, 2");
}

// ─── v12 behavior pinned beyond the TS suites ───────────────────────────
// Each expectation below was checked against v12 on the same input.

#[test]
fn min_is_clamped_to_one() {
    let unbounded = Repeat::new("r", digits(), at_least(0));
    let bounded_zero = Repeat::new("r", digits(), bounded(0, 3));
    assert_eq!(unbounded.min(), 1);
    assert_eq!(bounded_zero.min(), 1);

    // Clamped, a repeat with min 0 still fails on no match at all.
    assert!(parse(&mut unbounded.into(), "x").error.is_some());
    assert!(parse(&mut bounded_zero.into(), "x").error.is_some());
}

#[test]
fn a_zero_or_below_min_bound_becomes_min() {
    // v12: `max = Math.max(min, max || min)`.
    assert_eq!(Repeat::new("r", digits(), bounded(1, 0)).max(), Some(1));
    assert_eq!(Repeat::new("r", digits(), bounded(3, 2)).max(), Some(3));

    let mut once = rep("r", digits(), bounded(1, 0));
    assert_eq!(parse(&mut once, "12").nodes.unwrap()[0].4, "1");
}

#[test]
fn unbounded_failure_with_nothing_kept_is_the_childs_error() {
    // Reported on the child, from where the repeat started to where the
    // failing attempt began.
    let mut repeat = rep("r", digits(), RepeatOptions::default());
    assert_eq!(parse(&mut repeat, "x").error, Some(("digit", 0, 0)));

    // Inside a sequence the repeat starts at 1.
    let mut sequence = seq("s", vec![lit("a", "a"), repeat]);
    let mut cursor = Cursor::new("ax");
    assert!(sequence.parse(&mut cursor).is_none());
    assert_eq!(cursor.furthest_error().unwrap().pattern.name, "digit");
}

#[test]
fn a_composite_childs_failure_is_reported_on_the_child_itself() {
    // With a sequence child the attribution is visible: the inner literal
    // that failed (`b@1..1`) is not what the repeat reports. It re-records
    // the failure on its child, the sequence, from where the repeat started.
    let ab = || seq("s", vec![lit("a", "a"), lit("b", "b")]);

    let mut plain = rep("r", ab(), RepeatOptions::default());
    assert_eq!(parse(&mut plain, "ax").error, Some(("s", 0, 0)));

    let mut with_divider = rep("r", ab(), divided(lit("comma", ",")));
    assert_eq!(parse(&mut with_divider, "ax").error, Some(("s", 0, 0)));

    let mut inside = seq(
        "p",
        vec![lit("q", "q"), rep("r", ab(), RepeatOptions::default())],
    );
    assert_eq!(parse(&mut inside, "qax").error, Some(("s", 1, 1)));
}

#[test]
fn a_nodeless_divider_rewinds_before_the_next_child() {
    // The divider `(","?)+` returns no node and no error after consuming
    // ",,": the repeat moves back to where the divider began, so the next
    // "x" is never reached and only the first one is kept.
    let divider = || rep("dr", opt("o", lit("c", ",")), RepeatOptions::default());

    let mut unbounded = rep("r", lit("x", "x"), divided(divider()));
    assert_eq!(
        parse(&mut unbounded, "x,,x").nodes.unwrap(),
        vec![
            n(NodeKind::InfiniteRepeat, "r", 0, 1, "x"),
            n(NodeKind::Literal, "x", 0, 1, "x"),
        ]
    );
    assert!(!unbounded.test("x,,x"));

    let mut finite = rep(
        "r",
        lit("x", "x"),
        RepeatOptions {
            max: Some(3),
            ..divided(divider())
        },
    );
    assert_eq!(
        parse(&mut finite, "x,,x").nodes.unwrap(),
        vec![
            n(NodeKind::FiniteRepeat, "r", 0, 1, "x"),
            n(NodeKind::Literal, "x", 0, 1, "x"),
        ]
    );
}

#[test]
fn a_nodeless_bounded_slot_rewinds_before_the_next_slot() {
    // The child `(a?)+` consumes "aa" yet returns no node (its third try
    // matched nothing). The slot rewinds to 0, where the divider fails: no
    // node at all. Without the rewind the divider would match at 2.
    let mut repeat = rep(
        "r",
        rep("cr", opt("o", lit("a", "a")), RepeatOptions::default()),
        RepeatOptions {
            max: Some(3),
            ..divided(lit("c", ","))
        },
    );
    let outcome = parse(&mut repeat, "aa,");
    assert!(outcome.nodes.is_none());
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.pos, 0);
}

#[test]
fn a_min_failure_after_a_partial_literal_reports_where_it_stopped() {
    // v12's Literal leaves its cursor on the character that didn't match,
    // and the repeat's own error reports from there.
    let mut unbounded = rep("r", lit("ab", "ab"), at_least(2));
    assert_eq!(parse(&mut unbounded, "abac").error, Some(("r", 0, 3)));

    let mut finite = rep("r", lit("ab", "ab"), bounded(1, 3));
    assert_eq!(parse(&mut finite, "ac").error, Some(("r", 0, 1)));

    let mut long_divider = rep(
        "r",
        lit("a", "a"),
        RepeatOptions {
            min: 2,
            ..divided(lit("cc", ",,"))
        },
    );
    assert_eq!(parse(&mut long_divider, "a,x").error, Some(("r", 0, 2)));
}

#[test]
fn a_min_failure_after_a_sequence_ran_out_of_text_reports_the_end() {
    // The second "a"+"b" runs out of text after "aa"; v12's sequence stays
    // at the end, so the repeat's error reaches 4, not 3 where it began.
    let mut repeat = rep(
        "o",
        seq(
            "s",
            vec![
                rep("r", lit("a", "a"), RepeatOptions::default()),
                lit("b", "b"),
            ],
        ),
        RepeatOptions {
            min: 2,
            ..divided(lit("c", ","))
        },
    );
    let outcome = parse(&mut repeat, "ab,aa");
    assert_eq!(outcome.error, Some(("o", 0, 4)));
    assert_eq!(outcome.pos, 0);
}

#[test]
fn unbounded_min_failure_is_the_repeats_error() {
    let mut repeat = rep("r", lit("a", "a"), at_least(3));

    // Mid-text: `last` is where the failed attempt began.
    assert_eq!(parse(&mut repeat, "aab").error, Some(("r", 0, 2)));
    // At the end: `last` is the last character, as v12's inclusive cursor
    // reports it.
    assert_eq!(parse(&mut repeat, "aa").error, Some(("r", 0, 1)));

    let mut divided_min = rep(
        "r",
        digits(),
        RepeatOptions {
            min: 3,
            ..divided(lit("comma", ","))
        },
    );
    // Dividers don't count toward min: "1,2," is two matches.
    assert_eq!(parse(&mut divided_min, "1,2,").error, Some(("r", 0, 3)));
    assert_eq!(parse(&mut divided_min, "1,2,x").error, Some(("r", 0, 4)));
}

#[test]
fn bounded_min_failure_is_the_repeats_error() {
    let mut repeat = rep("r", digits(), bounded(3, 3));

    let mid = parse(&mut repeat, "12b");
    assert_eq!(mid.error, Some(("r", 0, 2)));
    assert_eq!(mid.pos, 0);
    assert_eq!(parse(&mut repeat, "12").error, Some(("r", 0, 1)));
}

#[test]
fn a_zero_width_child_without_a_divider_stops_the_loop() {
    // An optional child that stops matching would match "nothing" forever.
    // The loop ends instead, and v12 reports "no node, no error" even with
    // matches kept before it.
    let mut repeat = rep("r", opt("od", digits()), RepeatOptions::default());

    let outcome = parse(&mut repeat, "12b");
    assert!(outcome.nodes.is_none());
    assert!(outcome.error.is_none());
    assert_eq!(outcome.pos, 2);

    let all = parse(&mut repeat, "12");
    assert_eq!(all.nodes.unwrap()[0].4, "12");
}

#[test]
fn a_zero_width_child_with_a_divider_keeps_going_on_dividers() {
    let mut repeat = rep("r", opt("od", digits()), divided(lit("comma", ",")));

    let outcome = parse(&mut repeat, ",,1");
    assert_eq!(
        outcome.nodes.unwrap(),
        vec![
            n(NodeKind::InfiniteRepeat, "r", 0, 3, ",,1"),
            n(NodeKind::Literal, "comma", 0, 1, ","),
            n(NodeKind::Literal, "comma", 1, 2, ","),
            digit(2, "1"),
        ]
    );

    // Neither the child nor the divider matched: nothing kept, so it is a
    // min failure on the repeat.
    assert_eq!(parse(&mut repeat, "x").error, Some(("r", 0, 0)));
}

#[test]
fn a_trimmed_lone_divider_leaves_no_node_and_no_error() {
    let mut repeat = rep(
        "r",
        opt("od", digits()),
        RepeatOptions {
            trim_divider: true,
            ..divided(lit("comma", ","))
        },
    );

    let outcome = parse(&mut repeat, ",x");
    assert!(outcome.nodes.is_none());
    assert!(outcome.error.is_none());
    assert_eq!(outcome.pos, 0);
}

#[test]
fn a_bounded_repeat_with_a_divider_and_trim_divider() {
    let mut repeat = rep(
        "r",
        digits(),
        RepeatOptions {
            min: 1,
            max: Some(3),
            divider: Some(lit("comma", ",")),
            trim_divider: true,
        },
    );

    let trimmed = parse(&mut repeat, "1,x");
    assert_eq!(
        trimmed.nodes.unwrap(),
        vec![n(NodeKind::FiniteRepeat, "r", 0, 1, "1"), digit(0, "1")]
    );
    assert_eq!(trimmed.pos, 1, "the cursor gives the divider back");

    // At max, the last slot is a child: no divider slot follows it.
    let full = parse(&mut repeat, "1,2,3,4");
    assert_eq!(full.nodes.unwrap()[0].4, "1,2,3");
    assert_eq!(full.pos, 5);
}

#[test]
fn a_bounded_repeat_without_trim_keeps_its_last_divider() {
    let mut repeat = rep(
        "r",
        digits(),
        RepeatOptions {
            max: Some(2),
            ..divided(lit("comma", ","))
        },
    );

    let outcome = parse(&mut repeat, "1,2,3");
    assert_eq!(outcome.nodes.unwrap()[0].4, "1,2,");
    assert_eq!(outcome.pos, 4);
}

#[test]
fn a_trimmed_bounded_repeat_that_matches_nothing_fails_on_min() {
    // v12 throws a TypeError here (it pops a node that isn't there); with
    // nothing to trim this behaves like the untrimmed repeat.
    let mut repeat = rep("r", digits(), bounded_comma(1, 3));

    let outcome = parse(&mut repeat, "x");
    assert!(outcome.nodes.is_none());
    assert_eq!(outcome.error, Some(("r", 0, 0)));
}

#[test]
fn a_bounded_repeat_of_zero_width_matches_with_a_failed_divider_is_zero_width() {
    // v12 throws a TypeError here (no nodes, but an error left on the
    // cursor). The min was met by the zero-width match, so the failed
    // trailing divider is dropped like any trailing failure.
    let mut repeat = rep(
        "r",
        opt("od", digits()),
        RepeatOptions {
            max: Some(3),
            ..divided(lit("comma", ","))
        },
    );

    let outcome = parse(&mut repeat, "x");
    assert!(outcome.nodes.is_none());
    assert!(outcome.error.is_none());
    assert_eq!(outcome.pos, 0);
}

#[test]
fn the_loop_stops_at_the_end_of_the_text() {
    // Nothing is tried past the end: no divider error is recorded.
    let mut repeat = rep("r", digits(), divided(lit("comma", ",")));
    let mut cursor = Cursor::new("1,2");

    let id = repeat.parse(&mut cursor).unwrap();
    assert!(cursor.furthest_error().is_none());
    assert_eq!(cursor.pos(), 3);
    assert_eq!(cursor.into_tree(id).root().value(), "1,2");
}

#[test]
fn a_repeat_cannot_finish_a_sequence_the_text_ended_on() {
    // Sequence's end-of-text rule skips only an Optional, as in v12.
    let mut sequence = seq(
        "s",
        vec![rep("r", digits(), RepeatOptions::default()), lit("b", "b")],
    );

    assert!(sequence.test("12b"));
    let outcome = parse(&mut sequence, "12");
    assert!(outcome.nodes.is_none());
    assert_eq!(outcome.error, Some(("s", 0, 2)));
}

#[test]
fn graphemes_stay_whole_across_iterations() {
    // "e" matches the first byte of "é" (e + combining accent); the cursor
    // then sits at the end of that grapheme, so the loop stops there.
    let mut repeat = rep("r", lit("e", "e"), RepeatOptions::default());
    let mut cursor = Cursor::new("ee\u{301}");

    let id = repeat.parse(&mut cursor).unwrap();
    assert_eq!(cursor.pos(), 4);
    assert_eq!(cursor.into_tree(id).root().value(), "ee");
}

/// Matches any one character and, unlike the built-ins, records no match.
#[derive(Clone)]
struct AnyChar;

impl CustomParse for AnyChar {
    fn parse(&mut self, meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let start = cursor.pos();
        let Some(c) = cursor.rest().chars().next() else {
            cursor.record_error(start, start, meta);
            return None;
        };
        let end = start + c.len_utf8();
        cursor.resolve_error();
        cursor.set_pos(end);
        Some(cursor.leaf(NodeKind::Custom, meta.name, start, end))
    }

    fn clone_box(&self) -> Box<dyn CustomParse> {
        Box::new(self.clone())
    }
}

#[test]
fn the_unbounded_repeat_records_a_match_and_the_bounded_one_does_not() {
    // The child records no match of its own, so the leaf match shows
    // whether the repeat recorded one (v12: InfiniteRepeat does,
    // FiniteRepeat doesn't).
    let any_char = || -> Pattern { Custom::new("any", Box::new(AnyChar)).into() };

    let mut unbounded = rep("r", any_char(), RepeatOptions::default());
    let mut cursor = Cursor::new("ab");
    unbounded.parse(&mut cursor);
    assert_eq!(
        cursor
            .leaf_match()
            .map(|m| (m.pattern.name.as_str(), m.start, m.end)),
        Some(("r", 0, 2))
    );

    let mut bounded_repeat = rep("r", any_char(), up_to_three());
    let mut cursor = Cursor::new("ab");
    bounded_repeat.parse(&mut cursor);
    assert!(cursor.leaf_match().is_none());
}

// ─── Fix round 1: slot counts, trimming and termination ─────────────────
// Each expectation below was checked against v12 on the same input.

fn furthest(pattern: &mut Pattern, text: &str) -> Option<(String, usize, usize)> {
    let mut cursor = Cursor::new(text);
    pattern.parse(&mut cursor);
    cursor
        .furthest_error()
        .map(|e| (e.pattern.name.to_string(), e.start, e.last))
}

fn error_of(error: Option<cpat_core::ParseError>) -> Option<(String, usize, usize)> {
    error.map(|e| (e.pattern.name.to_string(), e.start, e.last))
}

/// Wraps a pattern and counts how often it is parsed, so a test of
/// termination asserts a step count. Past a generous bound it panics: a
/// runaway loop then fails the test instead of hanging it.
#[derive(Clone)]
struct Counted {
    inner: Pattern,
    calls: std::rc::Rc<std::cell::Cell<usize>>,
}

impl CustomParse for Counted {
    fn parse(&mut self, _meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        self.calls.set(self.calls.get() + 1);
        assert!(
            self.calls.get() <= 1_000,
            "the repeat keeps calling its child"
        );
        self.inner.parse(cursor)
    }

    fn clone_box(&self) -> Box<dyn CustomParse> {
        Box::new(self.clone())
    }
}

fn counted(inner: Pattern) -> (Pattern, std::rc::Rc<std::cell::Cell<usize>>) {
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let pattern = Custom::new(
        "counted",
        Box::new(Counted {
            inner,
            calls: calls.clone(),
        }),
    )
    .into();
    (pattern, calls)
}

#[test]
fn an_enormous_bound_neither_overflows_nor_shrinks_the_repeat() {
    // `2 * max` slots would overflow here (and wrap to 0 on wasm32, where a
    // JS max of 2^31 made the repeat fail everything on min).
    for trim_divider in [false, true] {
        let mut repeat = rep(
            "r",
            digits(),
            RepeatOptions {
                max: Some(usize::MAX),
                trim_divider,
                ..divided(lit("comma", ","))
            },
        );
        assert_eq!(parse(&mut repeat, "1,2").nodes.unwrap()[0].4, "1,2");
    }

    let mut undivided = rep("r", digits(), bounded(1, usize::MAX));
    assert_eq!(parse(&mut undivided, "123").nodes.unwrap()[0].4, "123");
}

#[test]
fn zero_width_slots_count_toward_min() {
    // v12 counts every zero-width child slot as a match: "aa" plus three
    // empty slots is five, which meets min 4.
    let mut repeat = rep("r", opt("oa", lit("a", "a")), bounded(4, 5));
    let outcome = parse(&mut repeat, "aab");
    assert_eq!(
        outcome.nodes.unwrap()[0],
        n(NodeKind::FiniteRepeat, "r", 0, 2, "aa")
    );
    assert_eq!(outcome.pos, 2);
    assert_eq!(furthest(&mut repeat, "aab"), Some(("a".into(), 2, 2)));

    let mut divided_by_optional = rep(
        "r",
        opt("oa", lit("a", "a")),
        RepeatOptions {
            min: 4,
            max: Some(5),
            ..divided(opt("oc", lit("c", ",")))
        },
    );
    let outcome = parse(&mut divided_by_optional, "a,ab");
    assert_eq!(
        outcome.nodes.unwrap()[0],
        n(NodeKind::FiniteRepeat, "r", 0, 3, "a,a")
    );
    assert_eq!(outcome.pos, 3);
    assert_eq!(
        furthest(&mut divided_by_optional, "a,ab"),
        Some(("c".into(), 3, 3))
    );

    // A literal divider that fails ends the walk: only three slots matched.
    let mut divided_by_literal = rep(
        "r",
        opt("oa", lit("a", "a")),
        RepeatOptions {
            min: 4,
            max: Some(5),
            ..divided(lit("c", ","))
        },
    );
    let outcome = parse(&mut divided_by_literal, "a,a,b");
    assert!(outcome.nodes.is_none());
    assert_eq!(outcome.error, Some(("r", 0, 4)));
}

#[test]
fn idle_laps_under_an_enormous_bound_end_the_walk() {
    // Every lap after "aa" matches nothing; the remaining child slots are
    // counted, not walked: the child runs three times ("a", "a", the idle
    // lap), not usize::MAX times.
    let (child, calls) = counted(opt("oa", lit("a", "a")));
    let mut repeat = rep("r", child, bounded(4, usize::MAX));
    let outcome = parse(&mut repeat, "aab");
    assert_eq!(
        outcome.nodes.unwrap()[0],
        n(NodeKind::FiniteRepeat, "r", 0, 2, "aa")
    );
    assert_eq!(outcome.pos, 2);
    assert_eq!(calls.get(), 3);

    // With a divider, the idle lap is a child and a divider that both
    // matched nothing.
    let (child, calls) = counted(opt("oa", lit("a", "a")));
    let mut divided_by_optional = rep(
        "r",
        child,
        RepeatOptions {
            min: 4,
            max: Some(usize::MAX),
            ..divided(opt("oc", lit("c", ",")))
        },
    );
    let outcome = parse(&mut divided_by_optional, "a,ab");
    assert_eq!(outcome.nodes.unwrap()[0].4, "a,a");
    assert_eq!(outcome.pos, 3);
    assert_eq!(calls.get(), 3);
}

#[test]
fn a_bounded_trim_gives_the_divider_back_before_the_min_check() {
    // The trimmed divider's start becomes the cursor position, and so the
    // `last` of the min failure: r@0..1, not r@0..2.
    let mut repeat = rep(
        "r",
        re("d", r"\d"),
        RepeatOptions {
            min: 2,
            max: Some(3),
            divider: Some(lit("c", ",")),
            trim_divider: true,
        },
    );

    let outcome = parse(&mut repeat, "1,a");
    assert!(outcome.nodes.is_none());
    assert_eq!(outcome.error, Some(("r", 0, 1)));
    assert_eq!(outcome.pos, 0);
    assert_eq!(furthest(&mut repeat, "1,a"), Some(("d".into(), 2, 2)));
}

#[test]
fn a_bounded_trim_has_no_divider_slot_after_the_last_child() {
    // With max 2 the slots are child, divider, child: no divider is tried
    // after "2", so no divider error is recorded at 3.
    let mut repeat = rep(
        "r",
        re("d", r"\d"),
        RepeatOptions {
            max: Some(2),
            divider: Some(lit("c", ",")),
            trim_divider: true,
            ..RepeatOptions::default()
        },
    );

    let outcome = parse(&mut repeat, "1,2x");
    assert_eq!(
        outcome.nodes.unwrap()[0],
        n(NodeKind::FiniteRepeat, "r", 0, 3, "1,2")
    );
    assert_eq!(outcome.pos, 3);
    assert_eq!(furthest(&mut repeat, "1,2x"), None);

    let result = repeat.exec("1,2x");
    assert!(result.tree.is_none());
    assert_eq!(error_of(result.furthest_error), Some(("r".into(), 3, 4)));
}

#[test]
fn an_unbounded_repeat_without_trim_keeps_its_last_divider() {
    let mut repeat = rep("r", re("d", r"\d"), divided(lit("c", ",")));

    let outcome = parse(&mut repeat, "1,");
    assert_eq!(
        outcome.nodes.unwrap()[..2],
        [
            n(NodeKind::InfiniteRepeat, "r", 0, 2, "1,"),
            n(NodeKind::Regex, "d", 0, 1, "1"),
        ]
    );
    assert_eq!(outcome.pos, 2);
    assert!(repeat.test("1,"));
}

#[test]
fn an_unbounded_lap_where_neither_child_nor_divider_consumes_ends_the_loop() {
    // Both are optional: without the "nothing moved" exit this would loop
    // forever. The child runs once; then no node, a min failure on the
    // repeat, and the cursor back where it started.
    let (child, calls) = counted(opt("oa", lit("a", "a")));
    let mut repeat = rep("r", child, divided(opt("oc", lit("c", ","))));

    let outcome = parse(&mut repeat, "b");
    assert!(outcome.nodes.is_none());
    assert_eq!(outcome.error, Some(("r", 0, 0)));
    assert_eq!(outcome.pos, 0);
    assert_eq!(calls.get(), 1);
    assert_eq!(furthest(&mut repeat, "b"), Some(("a".into(), 0, 0)));

    // After real matches, the idle lap ends the loop and the matches stand.
    calls.set(0);
    let outcome = parse(&mut repeat, "a,ab");
    assert_eq!(
        outcome.nodes.unwrap()[0],
        n(NodeKind::InfiniteRepeat, "r", 0, 3, "a,a")
    );
    assert_eq!(outcome.pos, 3);
    assert_eq!(calls.get(), 3);
    assert_eq!(furthest(&mut repeat, "a,ab"), Some(("c".into(), 3, 3)));
}

// ─── Fix round 2: where the cursor stays, end-of-text stops, progress ───
// Each expectation below was checked against v12 on the same input.

/// `(a?)+`: an unbounded repeat that ends with "no node, no error" after
/// it has moved the cursor (v12's own quirk, kept).
fn optional_as_plus() -> Pattern {
    rep("i", opt("oa", lit("a", "a")), RepeatOptions::default())
}

#[test]
fn an_unbounded_repeat_leaves_the_cursor_where_a_nodeless_child_left_it() {
    // v12 InfiniteRepeat never rewinds after a child that returned null with
    // no error: the divider is tried from where the child stopped.
    let mut repeat = rep("o", optional_as_plus(), divided(lit("c", ",")));

    let outcome = parse(&mut repeat, "aa,");
    assert_eq!(
        outcome.nodes.unwrap(),
        vec![
            n(NodeKind::InfiniteRepeat, "o", 0, 3, "aa,"),
            n(NodeKind::Literal, "c", 2, 3, ","),
        ]
    );
    assert_eq!(outcome.error, None);
    assert_eq!(furthest(&mut repeat, "aa,"), Some(("a".into(), 2, 2)));

    let outcome = parse(&mut repeat, "a,a,");
    assert_eq!(
        outcome.nodes.unwrap(),
        vec![
            n(NodeKind::InfiniteRepeat, "o", 0, 4, "a,a,"),
            n(NodeKind::Literal, "c", 1, 2, ","),
            n(NodeKind::Literal, "c", 3, 4, ","),
        ]
    );

    let outcome = parse(&mut repeat, "aa,b");
    assert_eq!(
        outcome.nodes.unwrap()[0],
        n(NodeKind::InfiniteRepeat, "o", 0, 3, "aa,")
    );
    assert_eq!(furthest(&mut repeat, "aa,b"), Some(("a".into(), 3, 3)));
}

#[test]
fn without_a_divider_a_nodeless_child_ends_the_loop_where_it_left_the_cursor() {
    let mut repeat = rep("o", optional_as_plus(), RepeatOptions::default());

    let outcome = parse(&mut repeat, "aab");
    assert!(outcome.nodes.is_none());
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.pos, 2);
}

#[test]
fn a_bounded_repeat_stops_at_the_end_of_the_text() {
    // v12 FiniteRepeat breaks on `!cursor.hasNext()`: no slot runs past the
    // end, so none records an error there.
    let mut plain = rep(
        "r",
        re("d", "\\d"),
        RepeatOptions {
            max: Some(3),
            ..RepeatOptions::default()
        },
    );
    assert_eq!(
        parse(&mut plain, "12").nodes.unwrap()[0],
        n(NodeKind::FiniteRepeat, "r", 0, 2, "12")
    );
    assert_eq!(furthest(&mut plain, "12"), None);

    let mut with_divider = rep(
        "r",
        re("d", "\\d"),
        RepeatOptions {
            max: Some(3),
            ..divided(lit("c", ","))
        },
    );
    // Stopped after a child, and after a divider.
    assert_eq!(furthest(&mut with_divider, "1"), None);
    assert_eq!(furthest(&mut with_divider, "1,"), None);
    assert_eq!(
        parse(&mut with_divider, "1,").nodes.unwrap()[0],
        n(NodeKind::FiniteRepeat, "r", 0, 2, "1,")
    );
}

#[test]
fn an_unbounded_repeat_stops_at_the_end_of_the_text_after_a_divider() {
    let mut repeat = rep("r", re("d", "\\d"), divided(lit("c", ",")));

    assert_eq!(furthest(&mut repeat, "1,"), None);
    assert_eq!(furthest(&mut repeat, "1,2,"), None);
    assert_eq!(
        parse(&mut repeat, "1,2,").nodes.unwrap()[0],
        n(NodeKind::InfiniteRepeat, "r", 0, 4, "1,2,")
    );
}

/// A custom pattern that "matches" zero-width wherever it is: it breaks
/// the contract, which asks for `None` instead.
#[derive(Clone)]
struct ZeroWidth {
    calls: std::rc::Rc<std::cell::Cell<usize>>,
}

impl CustomParse for ZeroWidth {
    fn parse(&mut self, meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        self.calls.set(self.calls.get() + 1);
        assert!(
            self.calls.get() <= 1_000,
            "the repeat keeps calling its child"
        );
        let pos = cursor.pos();
        Some(cursor.leaf(NodeKind::Custom, meta.name, pos, pos))
    }

    fn clone_box(&self) -> Box<dyn CustomParse> {
        Box::new(self.clone())
    }
}

fn zero_width() -> (Pattern, std::rc::Rc<std::cell::Cell<usize>>) {
    let calls = std::rc::Rc::new(std::cell::Cell::new(0));
    let pattern = Custom::new(
        "zw",
        Box::new(ZeroWidth {
            calls: calls.clone(),
        }),
    )
    .into();
    (pattern, calls)
}

#[test]
fn a_custom_zero_width_node_is_a_zero_width_success_so_a_repeat_ends() {
    // Kept as a node, it would make the repeat loop without progress until
    // memory ran out. Taken as the contract's zero-width success, the loop
    // ends after one call, as for any child that matches nothing.
    let (child, calls) = zero_width();
    let mut repeat = rep("r", child, RepeatOptions::default());

    let outcome = parse(&mut repeat, "abc");
    assert!(outcome.nodes.is_none());
    assert_eq!(outcome.error, None);
    assert_eq!(outcome.pos, 0);
    assert_eq!(calls.get(), 1);

    // An enormous bound takes the idle-lap exit after one call too.
    let (child, calls) = zero_width();
    let mut bounded = rep(
        "r",
        child,
        RepeatOptions {
            max: Some(usize::MAX),
            ..RepeatOptions::default()
        },
    );
    assert!(parse(&mut bounded, "abc").nodes.is_none());
    assert_eq!(calls.get(), 1);
}
