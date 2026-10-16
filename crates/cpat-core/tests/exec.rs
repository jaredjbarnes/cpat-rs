mod common;

use common::*;
use cpat_core::{Pattern, RepeatOptions};

// v12's `test(text)` is true exactly when `exec(text)` matches the whole
// text, "" included (v12's `fix(test)!` commit; before it exec returned
// early on "" and `test("")` was true for every pattern). Checked against
// v12 on the same patterns.
#[test]
fn empty_text_fails_a_pattern_that_needs_text_with_an_error_at_0() {
    let mut patterns = vec![
        lit("a", "a"),
        re("digits", r"\d+"),
        rep("as", lit("a", "a"), RepeatOptions::default()),
        seq("ab", vec![lit("a", "a"), lit("b", "b")]),
        seq("a?b", vec![opt("a?", lit("a", "a")), lit("b", "b")]),
        not("not-a?", opt("a?", lit("a", "a"))),
        // A sequence with no node fails unless every part is optional,
        // mid-text too.
        seq(
            "!a b?",
            vec![not("not-a", lit("a", "a")), opt("b?", lit("b", "b"))],
        ),
        choice("a|b", vec![lit("a", "a"), lit("b", "b")]),
        // An ordered choice skips an alternative that matched nothing.
        choice("a|b?", vec![lit("a", "a"), opt("b?", lit("b", "b"))]),
        context("ctx", reference("x"), vec![lit("x", "x")]),
    ];

    for pattern in &mut patterns {
        let name = pattern.name().to_string();
        assert!(!pattern.test(""), "{name}");
        assert!(!pattern.test_recording(""), "{name}");
        let result = pattern.exec("");
        assert!(result.tree.is_none(), "{name}");
        assert!(!result.passed(), "{name}");
        let error = result.error.expect("an error");
        assert_eq!((error.start, error.last), (0, 0), "{name}");
        assert_eq!(
            result.furthest_error.expect("a furthest error").start,
            0,
            "{name}"
        );
    }
}

#[test]
fn empty_text_matches_a_pattern_that_accepts_it_with_no_tree_and_no_error() {
    let mut patterns = vec![
        opt("a?", lit("a", "a")),
        seq(
            "a?b?",
            vec![opt("a?", lit("a", "a")), opt("b?", lit("b", "b"))],
        ),
        rep(
            "as",
            lit("a", "a"),
            RepeatOptions {
                min: 0,
                ..RepeatOptions::default()
            },
        ),
        rep(
            "as",
            lit("a", "a"),
            RepeatOptions {
                min: 0,
                max: Some(2),
                ..RepeatOptions::default()
            },
        ),
        not("not-a", lit("a", "a")),
        context("ctx", reference("x"), vec![opt("x", lit("y", "y"))]),
    ];

    for pattern in &mut patterns {
        let name = pattern.name().to_string();
        assert!(pattern.test(""), "{name}");
        assert!(pattern.test_recording(""), "{name}");
        let result = pattern.exec("");
        assert!(result.tree.is_none(), "{name}");
        assert!(result.passed(), "{name}");
        assert!(result.error.is_none(), "{name}");
    }
}

#[test]
fn empty_text_agrees_with_an_empty_rest_mid_text() {
    let between = |pattern: Pattern| seq("between", vec![lit("<", "<"), pattern, lit(">", ">")]);
    let patterns = vec![
        lit("a", "a"),
        opt("a?", lit("a", "a")),
        not("not-a", lit("a", "a")),
        not("not-a?", opt("a?", lit("a", "a"))),
        choice("a|b?", vec![lit("a", "a"), opt("b?", lit("b", "b"))]),
        seq(
            "!a b?",
            vec![not("not-a", lit("a", "a")), opt("b?", lit("b", "b"))],
        ),
        rep(
            "as",
            lit("a", "a"),
            RepeatOptions {
                min: 0,
                ..RepeatOptions::default()
            },
        ),
    ];

    for pattern in patterns {
        let name = pattern.name().to_string();
        let alone = pattern.clone().test("");
        assert_eq!(alone, between(pattern).test("<>"), "{name}");
    }
}

#[test]
fn empty_text_is_recorded() {
    let mut pattern = seq("ab", vec![lit("a", "a"), lit("b", "b")]);
    let result = pattern.exec_recording("");

    assert!(result.is_recording());
    assert!(!result.errors().is_empty());
    assert!(result.errors().iter().all(|error| error.start == 0));
}

#[test]
fn test_fails_on_any_error_and_passes_a_full_match() {
    let mut a = lit("a", "a");

    assert!(a.test("a"));
    assert!(!a.test("b"));
    assert!(!a.test("ab"));
}

#[test]
fn test_fails_when_the_pattern_cannot_run() {
    let mut pattern = seq("s", vec![reference_to("r", "nowhere")]);

    assert!(!pattern.test("a"));
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

// ─── A match must be the text itself, every code point (v12's isMatch) ──
// A literal can match the base of a grapheme (`~` of `~` + combining
// accent); the next pattern starts after the whole grapheme, so the accent
// belongs to no node. v12's exec compares the joined values with the text
// and reports no match. Each expectation below was checked against v12
// (UTF-16 offsets converted to bytes).

fn tilde_digit() -> Pattern {
    seq("s", vec![lit("tilde", "~"), re("digit", r"\d")])
}

#[test]
fn a_mark_no_node_covers_is_no_match() {
    let mut pattern = tilde_digit();

    // v12: no match, error and furthest error [2, 3]: from the value's
    // length ("~5", 2 units, which is the `5` at byte 3) to the end.
    assert_eq!(
        exec_outline(&mut pattern, "~\u{301}5"),
        outlined("-", "s@3..4", "s@3..4")
    );
    assert!(!pattern.test("~\u{301}5"));
    assert!(pattern.exec("~5").is_match());
}

#[test]
fn a_partial_match_still_starts_its_error_at_the_values_end() {
    let mut pattern = seq("s", vec![lit("a", "a")]);

    assert_eq!(
        exec_outline(&mut pattern, "ab"),
        outlined("-", "s@1..2", "s@1..2")
    );
    assert_eq!(
        exec_outline(&mut pattern, "a\u{301}b"),
        outlined("-", "s@1..4", "s@1..4")
    );
}

#[test]
fn deviation_a_value_length_inside_a_surrogate_pair_starts_at_the_pair() {
    // v12: [2, 4]. The value "~5" is 2 units long, and unit 2 is the second
    // half of the skin-tone modifier: no byte offset names it, so the error
    // starts at the modifier (unit 1). DESIGN §5.
    let mut pattern = tilde_digit();

    assert_eq!(
        exec_outline(&mut pattern, "~\u{1F3FD}5"),
        outlined("-", "s@1..6", "s@1..6")
    );
}
