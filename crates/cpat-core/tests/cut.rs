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

#[test]
fn take_until_a_cut_inside_a_failed_terminator_probe_does_not_commit() {
    // Cut.test.ts:
    //   scan = ?->| ("x" ~/ "y")
    //   committed = scan + "!"
    //   choice = committed | "axq"
    // Probing for `xy` at the `x` fires the cut and fails at `q`; the probe
    // is discarded and the scan runs on to the end of the text, where `!`
    // fails. Only if the probe's cut was thrown away can `axq` match.
    let scan = take_until("scan", seq("xy", vec![lit("x", "x"), cut(), lit("y", "y")]));
    let committed = seq("committed", vec![scan, lit("bang", "!")]);
    let mut choice = choice("choice", vec![committed, lit("axq", "axq")]);

    assert!(choice.test("axq"));
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

// Cut.test.ts "clone preserves the cut (Reference resolution path)": a
// reference resolves by cloning, and the clone must still be the same cut.
#[test]
fn clone_preserves_the_cut() {
    let cut = cut();
    let clone = cut.clone();

    assert!(matches!(clone, Pattern::Cut(_)));
    assert_eq!(clone.id(), cut.id());
}

// Cut.test.ts "cut works inside an Expression's alternatives". Every
// alternative is an atom (none starts and ends with `expr`), so in v12 this
// is the ordered choice too, with the cut reached through references.
//
//   group = "(#" ~/ expr ")"
//   pair  = "(" expr expr ")"
//   expr  = group | pair | atom
fn cut_through_references() -> Pattern {
    let group = seq(
        "group",
        vec![
            lit("open", "(#"),
            cut(),
            reference("expr"),
            lit("close", ")"),
        ],
    );
    let pair = seq(
        "pair",
        vec![
            lit("open", "("),
            reference("expr"),
            reference("expr"),
            lit("close", ")"),
        ],
    );
    choice("expr", vec![group, pair, re("atom", "[a-z]")])
}

#[test]
fn cut_works_inside_a_recursive_choices_alternatives() {
    let mut expr = cut_through_references();

    assert!(expr.test("(#a)"));
    assert!(expr.test("(ab)"));
    // `(#` commits: the failure inside `group` is not retried as `pair`.
    assert!(!expr.test("(#1)"));
}

#[test]
fn a_cut_commits_only_its_own_recursion_level() {
    // The inner `(#1)` fails after its cut. That commits the inner `expr`
    // only; the outer `pair` still fails on its own, with v12's errors.
    let mut expr = cut_through_references();

    assert_eq!(
        exec_outline(&mut expr, "((#a)b)").0,
        "pair@0..7[open@0..1 group@1..5[open@1..3 atom@3..4 close@4..5] atom@5..6 close@6..7]"
    );
    assert_eq!(
        exec_outline(&mut expr, "(a(#1))"),
        outlined("-", "expr@0..7", "open@4..4")
    );
}

// ─── Cut in a choice's Pratt mode ───────────────────────────────────────
// The grammars below are built as v12's `Grammar.parseString` builds them
// (the named rules inlined, the whole wrapped in a context named after the
// rule), so the names in errors are v12's.

/// `word = /[a-z0-9]+/`, `arith-cmd = "((" ~/ word + "))"`,
/// `and-expr = expr + "&&" + expr`, `expr = and-expr | arith-cmd | word`.
fn shell_statement() -> Pattern {
    let word = re("word", "[a-z0-9]+");
    choice(
        "expr",
        vec![
            seq(
                "and-expr",
                vec![reference("expr"), lit("&&", "&&"), reference("expr")],
            ),
            seq(
                "arith-cmd",
                vec![lit("((", "(("), cut(), word.clone(), lit("))", "))")],
            ),
            word,
        ],
    )
}

#[test]
fn commitment_is_discharged_by_success() {
    // `((1))` commits at `((` and succeeds; the choice then probes for an
    // infix continuation (`&&`). Failing to find one is ordinary "no
    // continuation", not a committed failure of the whole statement.
    let mut expr = shell_statement();

    assert!(expr.test("((1))"));
    assert!(expr.test("((1))&&yes"));
    assert!(expr.test("a&&b"));
    assert_eq!(
        exec_shape(&mut expr, "((1))&&yes"),
        "and-expr:expression@0..10[arith-cmd:sequence@0..5[((:literal@0..2 word:regex@2..3 \
         )):literal@3..5] &&:literal@5..7 word:regex@7..10]"
    );

    // A committed failure still is one: bad content after `((` is an error,
    // not a fallthrough to `word`.
    assert!(!expr.test("((!bad))"));
    assert_eq!(
        exec_outline(&mut expr, "((!bad))"),
        outlined("-", "expr@0..8", "word@2..2")
    );
}

/// `expr = expr ~/ "+" + expr | "a"`.
fn cut_leading_infix() -> Pattern {
    let expr = choice(
        "expr",
        vec![
            seq(
                "anonymous-pattern-0",
                vec![reference("expr"), cut(), lit("+", "+"), reference("expr")],
            ),
            lit("a", "a"),
        ],
    );
    context("expr", expr, vec![])
}

#[test]
fn a_cut_leading_an_infix_operator_fires_after_the_last_operand_too() {
    // `expr ~/ "+" expr` commits, after EVERY operand, to continuing with
    // `+`. Mid-text a `;` after the operand is a committed failure; with no
    // text after the operand the cut is reached all the same, so the end of
    // the text is a committed failure too, the error just past the operand.
    let mut expr = cut_leading_infix();

    assert_eq!(
        parse_shape(&mut expr, "a;"),
        outlined("-", "+@1..1", "+@1..1")
    );
    assert_eq!(
        parse_shape(&mut expr, "a"),
        outlined("-", "+@1..1", "+@1..1")
    );
    assert_eq!(
        parse_shape(&mut expr, "a+a"),
        outlined("-", "+@3..3", "+@3..3")
    );
}

#[test]
fn a_cut_leading_a_postfix_operator_fires_at_the_end_too() {
    let mut expr = choice(
        "expr",
        vec![
            seq("bang", vec![reference("expr"), cut(), lit("!", "!")]),
            lit("a", "a"),
        ],
    );

    assert_eq!(
        exec_outline(&mut expr, "a"),
        outlined("-", "expr@0..1", "!@1..1")
    );
    assert_eq!(
        exec_outline(&mut expr, "a!!"),
        outlined("-", "expr@0..3", "!@3..3")
    );
}

#[test]
fn deviation_the_end_of_text_operator_error_sits_at_the_end_of_a_wide_last_grapheme() {
    // v12 records it at `index + 1`, one UTF-16 unit past the start of the
    // last grapheme: inside `😀` (two units). Positions here are grapheme
    // boundaries (DESIGN §5c), so it is the end of the text, as for a
    // sequence that runs out of text (DESIGN §11).
    let mut expr = context(
        "expr",
        choice(
            "expr",
            vec![
                seq(
                    "op",
                    vec![reference("expr"), cut(), lit("+", "+"), reference("expr")],
                ),
                lit("e", "😀"),
            ],
        ),
        vec![],
    );

    assert_eq!(
        parse_shape(&mut expr, "😀"),
        outlined("-", "+@4..4", "+@4..4")
    );
}

#[test]
fn operators_without_a_leading_cut_still_end_cleanly_at_the_end_of_the_text() {
    // `expr = expr + "+" ~/ expr | "a"`: the cut comes after `+`, so it is
    // never reached at the end of the text.
    let mut expr = choice(
        "expr",
        vec![
            seq(
                "anonymous-pattern-0",
                vec![reference("expr"), lit("+", "+"), cut(), reference("expr")],
            ),
            lit("a", "a"),
        ],
    );

    assert_eq!(
        exec_shape(&mut expr, "a+a"),
        "anonymous-pattern-0:expression@0..3[a:literal@0..1 +:literal@1..2 a:literal@2..3]"
    );
    assert_eq!(exec_outline(&mut expr, "a"), outlined("a@0..1", "-", "-"));
    // Past its cut, the operator commits: a missing right operand is an
    // error, not the `a` alone.
    assert_eq!(
        exec_outline(&mut expr, "a+b"),
        outlined("-", "expr@1..3", "a@2..2")
    );
}

#[test]
fn a_cut_inside_a_prefix_operator_commits_the_choice() {
    let mut expr = choice(
        "expr",
        vec![
            seq(
                "neg",
                vec![
                    lit("-", "-"),
                    cut(),
                    opt("o", lit("~", "~")),
                    reference("expr"),
                ],
            ),
            lit("a", "a"),
        ],
    );

    assert_eq!(
        exec_shape(&mut expr, "--a"),
        "neg:expression@0..3[-:literal@0..1 neg:expression@1..3[-:literal@1..2 a:literal@2..3]]"
    );
    assert_eq!(
        exec_outline(&mut expr, "-b"),
        outlined("-", "expr@0..2", "~@1..1")
    );
}

#[test]
fn a_cut_inside_an_operand_of_a_pratt_choice_commits_it() {
    let mut expr = choice(
        "expr",
        vec![
            seq(
                "add",
                vec![reference("expr"), lit("+", "+"), reference("expr")],
            ),
            seq(
                "grp",
                vec![lit("(", "("), cut(), reference("expr"), lit(")", ")")],
            ),
            lit("a", "a"),
        ],
    );

    assert_eq!(
        exec_shape(&mut expr, "(a)+a"),
        "add:expression@0..5[grp:sequence@0..3[(:literal@0..1 a:literal@1..2 ):literal@2..3] \
         +:literal@3..4 a:literal@4..5]"
    );
    assert_eq!(
        exec_outline(&mut expr, "a+(a"),
        outlined("-", "expr@0..4", "grp@2..4")
    );
    assert_eq!(
        exec_outline(&mut expr, "a+(b)"),
        outlined("-", "expr@0..5", "(@3..3")
    );
}

#[test]
fn a_cut_inside_a_successful_postfix_is_discharged() {
    // `call = expr "(" ~/ ")"`: after `a()` the text ends, and the operators
    // are tried at the end. The kept call's cut must not make that probe
    // (blocked at `(`) a committed failure.
    let mut expr = choice(
        "expr",
        vec![
            seq(
                "call",
                vec![reference("expr"), lit("(", "("), cut(), lit(")", ")")],
            ),
            lit("a", "a"),
        ],
    );

    assert_eq!(
        exec_shape(&mut expr, "a()()"),
        "call:expression@0..5[call:expression@0..3[a:literal@0..1 (:literal@1..2 ):literal@2..3] \
         (:literal@3..4 ):literal@4..5]"
    );
    assert_eq!(
        exec_outline(&mut expr, "a("),
        outlined("-", "expr@0..2", "call-postfix@1..2")
    );
}

#[test]
fn a_cut_inside_a_failed_prefix_keeps_its_deep_error() {
    // `neg = "-" ~/ "!" expr`: on `-a` the prefix commits at `-` and fails
    // at `!`. The choice fails right there, with that error, rather than
    // going on to try `-` as an operand.
    let mut expr = choice(
        "expr",
        vec![
            seq(
                "neg",
                vec![lit("-", "-"), cut(), lit("!", "!"), reference("expr")],
            ),
            lit("a", "a"),
        ],
    );

    assert_eq!(
        parse_shape(&mut expr, "-a"),
        outlined("-", "!@1..1", "!@1..1")
    );
    assert_eq!(
        parse_shape(&mut expr, "-!a").0,
        "neg:expression@0..3[-:literal@0..1 !:literal@1..2 a:literal@2..3]"
    );
}

#[test]
fn a_committed_alternative_that_matched_nothing_leaves_the_choice_matching_nothing() {
    // `p = ~/ "~"?` on text without `~` matches nothing: the cut fires and
    // the optional is absent. The choice is committed to it, so it too
    // matches nothing, with no error, and a sequence carries on past it (as
    // in v12). Milestone 1 recorded the choice's own error here instead.
    let committed = || seq("p", vec![cut(), opt("o", lit("~", "~"))]);
    let mut ordered = seq(
        "s",
        vec![choice("x", vec![committed(), lit("a", "a")]), lit("b", "b")],
    );
    let mut pratt = seq(
        "s",
        vec![
            choice(
                "expr",
                vec![
                    seq("p", vec![cut(), opt("o", lit("~", "~")), reference("expr")]),
                    lit("a", "a"),
                ],
            ),
            lit("b", "b"),
        ],
    );

    assert_eq!(
        exec_outline(&mut ordered, "b"),
        outlined("s@0..1[b@0..1]", "-", "~@0..0")
    );
    assert_eq!(
        exec_outline(&mut ordered, "ab"),
        outlined("-", "s@0..2", "~@0..0")
    );
    assert_eq!(
        exec_outline(&mut pratt, "b"),
        outlined("s@0..1[b@0..1]", "-", "~@0..0")
    );
}
