mod common;

use common::*;
use cpat_core::{Cursor, Pattern, RepeatOptions};

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

// ─── Only kept work commits: repeats ────────────────────────────────────
// Ported from Cut.test.ts "Cut — only kept work commits". The grammar there
// is built here with the direct API: `x*` is `Optional(Repeat(x))`, `x+` is
// `Repeat(x)`, `x{1,3}` is a bounded `Repeat`, `(x, sep)+` has a divider and
// `sep trim` sets trim_divider. Each fallback alternative matches if, and
// only if, the repeat throws away the cut along with the work it dropped.

/// `"x" ~/ "y"`
fn x_cut_y() -> Pattern {
    seq("xy", vec![lit("x", "x"), cut(), lit("y", "y")])
}

/// `item = "a" + ("x" ~/ "y")? + "!"`
fn item() -> Pattern {
    seq(
        "item",
        vec![lit("a", "a"), opt("xy?", x_cut_y()), lit("!", "!")],
    )
}

fn up_to_three() -> RepeatOptions {
    RepeatOptions {
        min: 1,
        max: Some(3),
        ..RepeatOptions::default()
    }
}

#[test]
fn infinite_repeat_star_a_cut_inside_a_dropped_iteration_does_not_commit() {
    // Cut.test.ts "InfiniteRepeat `*`: a cut inside a dropped iteration does
    // not commit"
    //   first = "w" + ("x" ~/ "y")* + "z";  first-choice = first | "wxq"
    let first = seq(
        "first",
        vec![
            lit("w", "w"),
            opt("star", rep("inner", x_cut_y(), RepeatOptions::default())),
            lit("z", "z"),
        ],
    );
    let mut first_choice = choice("first-choice", vec![first, lit("wxq", "wxq")]);
    assert!(first_choice.test("wxyxyz"));
    assert!(first_choice.test("wxq"));

    //   later = (item)* + "z";  later-choice = later | "a!axy?"
    let later = seq(
        "later",
        vec![
            opt("star", rep("inner", item(), RepeatOptions::default())),
            lit("z", "z"),
        ],
    );
    let mut later_choice = choice("later-choice", vec![later, lit("fallback", "a!axy?")]);
    assert!(later_choice.test("a!axy!z"));
    assert!(later_choice.test("a!axy?"));
}

#[test]
fn infinite_repeat_plus_a_cut_inside_a_dropped_trailing_iteration_does_not_commit() {
    // Cut.test.ts "InfiniteRepeat `+`: a cut inside a dropped trailing
    // iteration does not commit"
    //   committed = (item)+ + "z";  choice = committed | "a!axy?"
    let committed = seq(
        "committed",
        vec![
            rep("items", item(), RepeatOptions::default()),
            lit("z", "z"),
        ],
    );
    let mut choice = choice("choice", vec![committed, lit("fallback", "a!axy?")]);

    assert!(choice.test("a!axy!z"));
    assert!(choice.test("a!axy?"));
}

#[test]
fn infinite_repeat_a_cut_inside_a_kept_iteration_still_commits() {
    // Cut.test.ts "InfiniteRepeat: a cut inside a KEPT iteration still commits"
    //   committed = ("x" ~/ "y")+ + "z";  choice = committed | "xyxq"
    let committed = seq(
        "committed",
        vec![rep("r", x_cut_y(), RepeatOptions::default()), lit("z", "z")],
    );
    let mut choice = choice("choice", vec![committed, lit("xyxq", "xyxq")]);

    assert!(!choice.test("xyxq"));
}

#[test]
fn finite_repeat_a_cut_inside_a_dropped_iteration_does_not_commit() {
    // Cut.test.ts "FiniteRepeat: a cut inside a dropped iteration does not
    // commit"
    //   committed = (item){1,3} + "z";  choice = committed | "a!axy?"
    let committed = seq(
        "committed",
        vec![rep("items", item(), up_to_three()), lit("z", "z")],
    );
    let mut choice = choice("choice", vec![committed, lit("fallback", "a!axy?")]);

    assert!(choice.test("a!axy!z"));
    assert!(choice.test("a!axy?"));
}

#[test]
fn finite_repeat_a_cut_inside_a_kept_iteration_still_commits() {
    // Cut.test.ts "FiniteRepeat: a cut inside a KEPT iteration still commits"
    //   committed = ("x" ~/ "y"){1,3} + "z";  choice = committed | "xyxq"
    let committed = seq(
        "committed",
        vec![rep("r", x_cut_y(), up_to_three()), lit("z", "z")],
    );
    let mut choice = choice("choice", vec![committed, lit("xyxq", "xyxq")]);

    assert!(!choice.test("xyxq"));
}

#[test]
fn a_cut_inside_a_dropped_divider_does_not_commit() {
    // Cut.test.ts "repeat with a divider: a cut inside a dropped divider does
    // not commit"
    //   sep = "," ~/ ";";  committed = ("a", sep)+ + "!";  choice = committed | "a,?"
    let sep = seq("sep", vec![lit(",", ","), cut(), lit(";", ";")]);
    let committed = seq(
        "committed",
        vec![rep("r", lit("a", "a"), divided(sep)), lit("!", "!")],
    );
    let mut choice = choice("choice", vec![committed, lit("fallback", "a,?")]);

    assert!(choice.test("a,;a!"));
    assert!(choice.test("a,?"));
}

#[test]
fn a_cut_inside_a_trimmed_divider_does_not_commit() {
    // Cut.test.ts "repeat with a trimmed divider: a cut inside the trimmed
    // divider does not commit"
    //   sep = "," ~/ " "
    //   committed = ("a", sep trim)+ + "!";     choice = committed | "a, b"
    //   bounded = ("a", sep trim){1,3} + "!";   bounded-choice = bounded | "a, b"
    let trimmed = |max: Option<usize>| RepeatOptions {
        max,
        trim_divider: true,
        ..divided(seq("sep", vec![lit(",", ","), cut(), lit("space", " ")]))
    };
    let alternatives = |options: RepeatOptions| {
        vec![
            seq(
                "committed",
                vec![rep("r", lit("a", "a"), options), lit("!", "!")],
            ),
            lit("fallback", "a, b"),
        ]
    };

    let mut unbounded = choice("choice", alternatives(trimmed(None)));
    assert!(unbounded.test("a, a!"));
    assert!(unbounded.test("a, b"));

    let mut bounded = choice("bounded-choice", alternatives(trimmed(Some(3))));
    assert!(bounded.test("a, a!"));
    assert!(bounded.test("a, b"));
}

#[test]
fn a_cut_inside_a_kept_divider_still_commits() {
    // The counterpart of the dropped-divider case: the divider `,;` is kept,
    // so its cut commits even though the iteration after it fails, and the
    // fallback is never tried (v12 agrees: no match, error on "!" at 3).
    //   sep = "," ~/ ";";  committed = ("a", sep)+ + "!";  choice = committed | "a,;?"
    let alternatives = |max: Option<usize>| {
        let sep = seq("sep", vec![lit(",", ","), cut(), lit(";", ";")]);
        let options = RepeatOptions {
            max,
            ..divided(sep)
        };
        vec![
            seq(
                "committed",
                vec![rep("r", lit("a", "a"), options), lit("!", "!")],
            ),
            lit("fallback", "a,;?"),
        ]
    };

    for max in [None, Some(3)] {
        let mut choice = choice("choice", alternatives(max));
        let mut cursor = Cursor::new("a,;?");
        assert!(choice.parse(&mut cursor).is_none(), "max {max:?}");
        let error = cursor.error().unwrap();
        assert_eq!(
            (error.pattern.name.as_str(), error.start, error.last),
            ("!", 3, 3),
            "max {max:?}"
        );
        assert!(choice.test("a,;a!"), "max {max:?}");
    }
}
