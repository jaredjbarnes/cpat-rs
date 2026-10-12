mod common;

use common::*;
use cpat_core::{Cursor, Pattern};

/// A miniature of the shell's `$((arith))` vs `$(cmd)` ambiguity: both
/// alternatives share the `<<` prefix, and `<<#` commits to "numeric".
///
///   numeric = "<<#" ~/ digits ">>"
///   textual = "<<" word ">>"
///   choice  = numeric | textual
fn make_choice(with_cut: bool) -> Pattern {
    let mut numeric = vec![lit("open", "<<#")];
    if with_cut {
        numeric.push(cut());
    }
    numeric.push(re("digits", "[0-9]+"));
    numeric.push(lit("close", ">>"));

    let textual = seq(
        "textual",
        vec![lit("open", "<<"), re("word", "[a-z]+"), lit("close", ">>")],
    );

    choice("choice", vec![seq("numeric", numeric), textual])
}

fn parsed_name(pattern: &mut Pattern, text: &str) -> Option<&'static str> {
    let mut cursor = Cursor::new(text);
    let id = pattern.parse(&mut cursor)?;
    Some(cursor.into_tree(id).root().name())
}

#[test]
fn success_path_is_unchanged_by_the_cut() {
    assert_eq!(
        parsed_name(&mut make_choice(true), "<<#42>>"),
        Some("numeric")
    );
}

#[test]
fn an_alternative_failing_before_its_cut_still_falls_through() {
    assert_eq!(
        parsed_name(&mut make_choice(true), "<<abc>>"),
        Some("textual")
    );
}

#[test]
fn an_alternative_failing_after_its_cut_fails_the_whole_choice() {
    let mut choice = make_choice(true);
    let mut cursor = Cursor::new("<<#abc>>");

    assert!(choice.parse(&mut cursor).is_none());
    assert!(cursor.has_error());
}

#[test]
fn the_cut_prevents_reinterpretation_by_a_later_alternative() {
    let loose = seq(
        "loose",
        vec![
            lit("open", "<<"),
            re("anything", "[^>]+"),
            lit("close", ">>"),
        ],
    );
    let strict = |with_cut: bool| {
        let mut children = vec![lit("open", "<<#")];
        if with_cut {
            children.push(cut());
        }
        children.push(re("digit", "[0-9]"));
        children.push(lit("close", ">>"));
        seq("strict", children)
    };

    let mut committed = choice("choice", vec![strict(true), loose.clone()]);
    let mut backtracking = choice("choice", vec![strict(false), loose]);

    assert_eq!(parsed_name(&mut committed, "<<#x>>"), None);
    assert_eq!(parsed_name(&mut backtracking, "<<#x>>"), Some("loose"));
}

#[test]
fn a_cut_binds_to_the_nearest_enclosing_choice_only() {
    // inner = ("a" ~/ "1") | "a2"   — the cut commits INNER
    // outer = inner | "a2"          — outer still tries its own sibling
    let inner = choice(
        "inner",
        vec![
            seq("a1", vec![lit("a", "a"), cut(), lit("one", "1")]),
            lit("a2", "a2"),
        ],
    );
    let mut outer = choice("outer", vec![inner, lit("a2-outer", "a2")]);

    assert_eq!(parsed_name(&mut outer, "a2"), Some("a2-outer"));
}

#[test]
fn a_cut_with_no_enclosing_choice_does_nothing() {
    let mut sequence = seq("seq", vec![lit("a", "a"), cut(), lit("b", "b")]);

    assert!(sequence.test("ab"));

    let mut cursor = Cursor::new("ax");
    assert!(sequence.parse(&mut cursor).is_none());
    assert!(cursor.has_error());
}

#[test]
fn a_committed_failure_keeps_the_deep_error() {
    let mut choice = make_choice(true);
    let mut cursor = Cursor::new("<<#abc>>");

    choice.parse(&mut cursor);

    let error = cursor.error().unwrap();
    assert_eq!(
        error.pattern.name, "digits",
        "the error is the committed branch's, not the choice's"
    );
    assert_eq!(error.start, 3);
    assert!(cursor.furthest_error().unwrap().start >= 3);
}

#[test]
fn a_cut_reached_at_the_end_of_input_still_commits() {
    // `"a" ~/ "b" | "a"` on "a": the cut is reached right after "a", so the
    // missing "b" is a committed failure and the second alternative is never
    // tried. (The TS library skipped the cut at the end of input; v12 fixes it.)
    let mut committed = choice(
        "x",
        vec![
            seq("ab", vec![lit("a", "a"), cut(), lit("b", "b")]),
            lit("a-only", "a"),
        ],
    );
    assert!(!committed.test("a"));

    let mut uncommitted = choice(
        "x",
        vec![
            seq("ab", vec![lit("a", "a"), lit("b", "b")]),
            lit("a-only", "a"),
        ],
    );
    assert!(uncommitted.test("a"));
}

// ─── Only kept work commits ─────────────────────────────────────────────

#[test]
fn a_cut_inside_a_discarded_optional_does_not_commit() {
    // ("x" ~/ "y")? + "z" | "xq"   on "xq"
    let committed = seq(
        "opt-committed",
        vec![
            opt("xy?", seq("xy", vec![lit("x", "x"), cut(), lit("y", "y")])),
            lit("z", "z"),
        ],
    );
    let mut choice = choice("choice", vec![committed, lit("xq", "xq")]);

    assert!(choice.test("xq"));
}

#[test]
fn a_cut_inside_a_kept_optional_still_commits() {
    // ("x" ~/ "y")? + "z" | "xyq"   on "xyq": "xy" is kept, so its cut stands.
    let committed = seq(
        "opt-committed",
        vec![
            opt("xy?", seq("xy", vec![lit("x", "x"), cut(), lit("y", "y")])),
            lit("z", "z"),
        ],
    );
    let mut choice = choice("choice", vec![committed, lit("xyq", "xyq")]);

    assert!(!choice.test("xyq"));
}

#[test]
fn a_cut_inside_a_lookahead_never_commits() {
    // !("x" ~/ "y") + "x" + "q" | "xy"   on "xy": the lookahead's child
    // matches, so the lookahead fails, but its cut must not commit.
    let lookahead = not(
        "not-xy",
        seq("xy", vec![lit("x", "x"), cut(), lit("y", "y")]),
    );
    let committed = seq(
        "not-committed",
        vec![lookahead, lit("x", "x"), lit("q", "q")],
    );
    let mut choice = choice("choice", vec![committed, lit("xy", "xy")]);

    assert!(choice.test("xy"));
}
