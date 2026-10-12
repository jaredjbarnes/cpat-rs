//! Ports of v12 `Cut.test.ts` that build their grammars from cpat text:
//! '`~/` in grammar files' (with the shell-shaped blow-up case),
//! 'commitment is discharged by success', 'only kept work commits' and
//! 'at the end of the input'. `tests/cut.rs` has the same behaviors on
//! hand-built patterns.
//!
//! Work is measured as errors recorded, never as time: a count of failed
//! attempts is the same on every machine.

use cpat_core::{Cursor, Grammar, GrammarOptions, Pattern, Patterns};

fn build(text: &str) -> Patterns {
    Grammar::parse_string(text, GrammarOptions::default()).expect("the grammar builds")
}

fn rule(text: &str, name: &str) -> Pattern {
    build(text).remove(name).expect("the rule exists")
}

// ─── `~/` in grammar files ──────────────────────────────────────────────

#[test]
fn cut_sequences_like_plus_and_commits_the_enclosing_choice() {
    let mut choice = rule(
        r#"
numeric = "<<#" ~/ /[0-9]+/ + ">>"
textual = "<<" + /[a-z]+/ + ">>"
choice = numeric | textual
    "#,
        "choice",
    );

    assert!(choice.test("<<#42>>"));
    assert!(choice.test("<<abc>>"));
    // Committed failure: `<<#` proves numeric; bad digits are an error,
    // not a fallthrough.
    assert!(!choice.test("<<#abc>>"));
}

#[test]
fn cut_mixes_with_plus_in_one_sequence() {
    let mut choice = rule(
        r#"
rule = "a" + "b" ~/ "c" + "d"
other = /[a-z]+/
choice = rule | other
    "#,
        "choice",
    );

    assert!(choice.test("abcd"));
    // Fails BEFORE the cut (at "b"): falls through to `other`.
    assert!(choice.test("axcd"));
    // Fails AFTER the cut (at "c"): committed, `other` is never tried.
    assert!(!choice.test("abxd"));
}

// ─── The shell-shaped case ──────────────────────────────────────────────
//
// A distilled `$((arith))` vs `$(script)` grammar in which the two
// alternatives contain EACH OTHER. On `$(($((…` that never closes, a failed
// arithmetic reading at one depth is re-read as a script, which re-tries
// both readings one level down: without the cut the work doubles per
// level. With the cut, `$((` commits to arithmetic, so each level is
// parsed once.

fn script(with_cut: bool) -> Pattern {
    let separator = if with_cut { "~/" } else { "+" };
    rule(
        &format!(
            r#"
number = /[0-9]+/
word = /[a-z]+/
arith-paren = "(" + arith + ")"
arith = subst | arith-paren | number
arith-sub = "$((" {separator} arith + "))"
cmd-sub = "$(" + script + ")"
subst = arith-sub | cmd-sub
group = "(" + script + ")"
item = subst | group | word
script = (item, /\s+/)+
      "#
        ),
        "script",
    )
}

/// The errors a parse of `$((` repeated 1 to 8 times records, per depth.
fn errors_by_depth(with_cut: bool) -> Vec<usize> {
    let mut pattern = script(with_cut);
    (1..=8)
        .map(|depth| {
            let text = "$((".repeat(depth);
            let result = pattern.exec_recording(&text);
            assert!(!result.is_match());
            result.errors().len()
        })
        .collect()
}

fn differences(counts: &[usize]) -> Vec<usize> {
    counts.windows(2).map(|pair| pair[1] - pair[0]).collect()
}

#[test]
fn both_grammars_accept_the_same_well_formed_inputs() {
    for with_cut in [true, false] {
        let mut pattern = script(with_cut);
        assert!(pattern.test("$((42))"));
        assert!(pattern.test("$(((42)))"));
        assert!(pattern.test("$(($((1))))"));
        assert!(pattern.test("$(a (b) $((2)))"));
        assert!(!pattern.test("$((a"));
    }
}

#[test]
fn with_the_cut_every_extra_level_costs_the_same_linear() {
    let counts = errors_by_depth(true);
    // v12 counts 5, 8, 11, … calls to `recordErrorAt`; its `test` records
    // the summary error through another method, which is one more here.
    assert_eq!(counts, [6, 9, 12, 15, 18, 21, 24, 27]);
    assert_eq!(differences(&counts), [3; 7]);
}

#[test]
fn without_the_cut_every_extra_level_costs_at_least_double_the_last_exponential() {
    let growth = differences(&errors_by_depth(false));
    for pair in growth.windows(2) {
        assert!(pair[1] >= 2 * pair[0], "{growth:?}");
    }
}

// ─── Commitment is discharged by success ────────────────────────────────

#[test]
fn a_committed_atoms_success_does_not_poison_later_operator_probing() {
    let mut expr = rule(
        r#"
word = /[a-z0-9]+/
arith-cmd = "((" ~/ word + "))"
and-expr = expr + "&&" + expr
expr = and-expr | arith-cmd | word
    "#,
        "expr",
    );

    assert!(expr.test("((1))"));
    assert!(expr.test("((1))&&yes"));
    assert!(expr.test("a&&b"));
    // Committed failure still works: bad content after `((` is an error,
    // not a fallthrough to `word`.
    assert!(!expr.test("((!bad))"));
}

// ─── Only kept work commits ─────────────────────────────────────────────

#[test]
fn a_cut_inside_a_swallowed_optional_does_not_commit() {
    let mut choice = rule(
        r#"
committed = ("x" ~/ "y")? + "z"
choice = committed | "xq"
    "#,
        "choice",
    );
    assert!(choice.test("xyz"));
    assert!(choice.test("z"));
    assert!(choice.test("xq"));
}

#[test]
fn a_cut_inside_a_kept_optional_still_commits() {
    let mut choice = rule(
        r#"
committed = ("x" ~/ "y")? + "z"
choice = committed | "xyq"
    "#,
        "choice",
    );
    assert!(!choice.test("xyq"));
}

#[test]
fn a_cut_inside_a_lookahead_whose_child_fails_does_not_commit() {
    let mut choice = rule(
        r#"
committed = !("x" ~/ "y") + "x" + "q"
choice = committed | "xy"
    "#,
        "choice",
    );
    assert!(choice.test("xq"));
    assert!(choice.test("xy"));
}

#[test]
fn a_cut_inside_a_lookahead_whose_child_matches_does_not_commit() {
    let mut choice = rule(
        r#"
committed = !("x" ~/ "y") + /[a-z]+/
choice = committed | "xy"
    "#,
        "choice",
    );
    assert!(choice.test("ab"));
    assert!(choice.test("xy"));
}

#[test]
fn a_cut_inside_a_dropped_zero_or_more_iteration_does_not_commit() {
    let mut patterns = build(
        r#"
first = "w" + ("x" ~/ "y")* + "z"
first-choice = first | "wxq"
item = "a" + ("x" ~/ "y")? + "!"
later = (item)* + "z"
later-choice = later | "a!axy?"
    "#,
    );
    assert!(patterns["first-choice"].test("wxyxyz"));
    assert!(patterns["first-choice"].test("wxq"));
    assert!(patterns["later-choice"].test("a!axy!z"));
    assert!(patterns["later-choice"].test("a!axy?"));
}

#[test]
fn a_cut_inside_a_dropped_trailing_one_or_more_iteration_does_not_commit() {
    let mut choice = rule(
        r#"
item = "a" + ("x" ~/ "y")? + "!"
committed = (item)+ + "z"
choice = committed | "a!axy?"
    "#,
        "choice",
    );
    assert!(choice.test("a!axy!z"));
    assert!(choice.test("a!axy?"));
}

#[test]
fn a_cut_inside_a_kept_iteration_still_commits() {
    let mut choice = rule(
        r#"
committed = ("x" ~/ "y")+ + "z"
choice = committed | "xyxq"
    "#,
        "choice",
    );
    assert!(!choice.test("xyxq"));
}

#[test]
fn a_cut_inside_a_dropped_bounded_iteration_does_not_commit() {
    let mut choice = rule(
        r#"
item = "a" + ("x" ~/ "y")? + "!"
committed = (item){1,3} + "z"
choice = committed | "a!axy?"
    "#,
        "choice",
    );
    assert!(choice.test("a!axy!z"));
    assert!(choice.test("a!axy?"));
}

#[test]
fn a_cut_inside_a_kept_bounded_iteration_still_commits() {
    let mut choice = rule(
        r#"
committed = ("x" ~/ "y"){1,3} + "z"
choice = committed | "xyxq"
    "#,
        "choice",
    );
    assert!(!choice.test("xyxq"));
}

#[test]
fn a_cut_inside_a_dropped_divider_does_not_commit() {
    let mut choice = rule(
        r#"
sep = "," ~/ ";"
committed = ("a", sep)+ + "!"
choice = committed | "a,?"
    "#,
        "choice",
    );
    assert!(choice.test("a,;a!"));
    assert!(choice.test("a,?"));
}

#[test]
fn a_cut_inside_a_trimmed_divider_does_not_commit() {
    let mut patterns = build(
        r#"
sep = "," ~/ " "
committed = ("a", sep trim)+ + "!"
bounded = ("a", sep trim){1,3} + "!"
choice = committed | "a, b"
bounded-choice = bounded | "a, b"
    "#,
    );
    assert!(patterns["choice"].test("a, a!"));
    assert!(patterns["choice"].test("a, b"));
    assert!(patterns["bounded-choice"].test("a, a!"));
    assert!(patterns["bounded-choice"].test("a, b"));
}

#[test]
fn a_cut_inside_a_failed_terminator_probe_does_not_commit() {
    let mut choice = rule(
        r#"
scan = ?->| ("x" ~/ "y")
committed = scan + "!"
choice = committed | "axq"
    "#,
        "choice",
    );
    assert!(choice.test("axq"));
}

// ─── At the end of the input ────────────────────────────────────────────

#[test]
fn a_cut_reached_at_the_end_of_the_input_fires() {
    let mut x = rule(r#"x = "a" ~/ "b" | "a""#, "x");
    let mut cursor = Cursor::new("a");
    assert!(x.parse(&mut cursor).is_none());
    // The deep error is the committed alternative's unfinished sequence,
    // past the `a`, not the choice's shallow one at index 0. (v12's
    // inclusive `lastIndex` 1 is the end of the text here.)
    let error = cursor.error().expect("the choice failed");
    assert_eq!(error.pattern.name.as_str(), "anonymous-pattern-0");
    assert_eq!((error.start, error.last), (0, 1));
    assert!(!x.test("a"));
}

#[test]
fn matches_the_committed_failure_when_text_does_follow() {
    let mut x = rule(r#"x = "a" ~/ "b" | "a" + "c" | "a""#, "x");
    assert!(!x.test("a"));
    assert!(!x.test("ac"));
    assert!(x.test("ab"));
}

#[test]
fn a_cut_with_only_optional_children_after_it_succeeds() {
    let mut x = rule(r#"x = "a" ~/ "b"? | "a""#, "x");
    let result = x.exec("a");
    let root = result.tree().expect("matches").root();
    assert_eq!(root.value(), "a");
    assert_eq!(root.kind(), cpat_core::NodeKind::Sequence);
}

#[test]
fn a_cut_after_a_child_that_still_needs_text_is_never_reached() {
    let mut x = rule(r#"x = "a" + "b" ~/ "c" | "a""#, "x");
    let result = x.exec("a");
    let root = result.tree().expect("matches").root();
    assert_eq!(root.kind(), cpat_core::NodeKind::Literal);
    assert_eq!(root.value(), "a");
}

#[test]
fn a_cut_leading_an_infix_operator_fires_after_the_last_operand_too() {
    let mut expr = rule(r#"expr = expr ~/ "+" + expr | "a""#, "expr");

    let mut mid_text = Cursor::new("a;");
    assert!(expr.parse(&mut mid_text).is_none());
    assert_eq!(mid_text.error().unwrap().pattern.name.as_str(), "+");

    let mut at_end = Cursor::new("a");
    assert!(expr.parse(&mut at_end).is_none());
    let error = at_end.error().unwrap();
    assert_eq!(error.pattern.name.as_str(), "+");
    assert_eq!(error.last, 1);

    let mut after_operators = Cursor::new("a+a");
    assert!(expr.parse(&mut after_operators).is_none());
}

#[test]
fn operators_without_a_leading_cut_still_end_cleanly_at_the_end_of_the_text() {
    let mut expr = rule(r#"expr = expr + "+" ~/ expr | "a""#, "expr");
    let value = |expr: &mut Pattern, text: &str| {
        expr.exec(text)
            .tree()
            .map(|tree| tree.root().value().to_string())
    };
    assert_eq!(value(&mut expr, "a+a").as_deref(), Some("a+a"));
    assert_eq!(value(&mut expr, "a").as_deref(), Some("a"));
    assert!(expr.exec("a").furthest_error.is_none());
}
