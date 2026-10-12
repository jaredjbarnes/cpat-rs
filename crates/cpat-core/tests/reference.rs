//! Ports of v12 `Reference.test.ts`, plus recursion and resolution cases.
//! Every expected outline was produced by running v12 on the same pattern
//! and input (`name@start..end`, TS inclusive `lastIndex` L printed as L+1).
//!
//! Deferred to the intellisense milestones: 'Get Tokens', 'Get Tokens After
//! With No Parent', 'Get Next Tokens*', 'Get Tokens After*', 'Get Patterns*',
//! 'Get Next Patterns*', 'Find Pattern' and 'Properties' (`parent`,
//! `children`), which need the introspection API.

mod common;

use common::*;
use cpat_core::{Cursor, Pattern, PatternError, Reference, RepeatOptions};

/// v12 `createValuePattern`: `value = number | array`, where an array is
/// `[` values `]` and the values refer back to `value`.
fn value_pattern() -> Pattern {
    let values = rep(
        "values",
        reference("value"),
        RepeatOptions {
            divider: Some(re("divider", r"\s*,\s+")),
            ..RepeatOptions::default()
        },
    );
    let array = seq(
        "array",
        vec![lit("open-bracket", "["), values, lit("close-bracket", "]")],
    );
    choice("value", vec![re("number", r"\d+"), array])
}

/// The `Reference` inside `value_pattern`, as v12's tests find it.
fn the_reference(value: &Pattern) -> Pattern {
    let Pattern::Choice(value) = value else {
        unreachable!()
    };
    let Pattern::Sequence(array) = &value.alternatives()[1] else {
        unreachable!()
    };
    let Pattern::Repeat(values) = &array.children()[1] else {
        unreachable!()
    };
    values.child().clone()
}

fn unknown(name: &str, target: &str) -> PatternError {
    PatternError::UnknownReference {
        name: name.to_string(),
        target: target.to_string(),
    }
}

// ─── Ported from Reference.test.ts ──────────────────────────────────────

#[test]
fn one_deep_successful_parse() {
    // v12 drives `parse` directly, with the reference finding `value` by
    // walking up to the root. Here the root's tree is searched by `exec`,
    // which builds that scope (DESIGN §3); the text is matched whole either
    // way, so the tree is the same.
    let mut value = value_pattern();

    let (tree, _, _) = exec_outline(&mut value, "[1, 2]");

    assert_eq!(
        tree,
        "array@0..6[open-bracket@0..1 values@1..5[number@1..2 divider@2..4 number@4..5] close-bracket@5..6]"
    );
}

#[test]
fn no_reference_pattern_is_an_error_value_not_a_panic() {
    // v12 throws. Driving `parse` directly with a name nothing defines is a
    // failure on the cursor instead, with the reason kept beside it.
    let mut bad: Pattern = Reference::new("bad-reference").into();
    let mut cursor = Cursor::new("text");

    assert!(bad.parse(&mut cursor).is_none());

    let error = cursor.error().unwrap();
    assert_eq!(
        (error.start, error.last, error.pattern.name.as_str()),
        (0, 0, "bad-reference")
    );
    assert_eq!(
        cursor.pattern_error(),
        Some(&unknown("bad-reference", "bad-reference"))
    );
}

#[test]
fn exec() {
    // v12 execs the reference while it still sits inside `value`, so it
    // resolves through its parents and "B" simply doesn't match. A Rust
    // pattern has no parents: a reference taken out of its tree names
    // nothing, and says so.
    let value = value_pattern();
    let mut reference = the_reference(&value);

    let result = reference.exec("B");

    assert!(result.tree.is_none());
    assert_eq!(result.pattern_error, Some(unknown("value", "value")));
}

#[test]
fn exec_after_resolving_still_reports_the_inner_name() {
    // Once the reference has resolved, its clones keep the master it found
    // (v12 `_cachedPattern`), so the clone itself resolves. But that master
    // refers to `value` again, and outside the tree nothing defines it. v12
    // throws as soon as an input reaches that inner reference ("[1, [2]]");
    // here it is reported up front, whatever the input.
    let mut value = value_pattern();
    assert!(value.test("[1]"));
    let mut reference = the_reference(&value);

    let result = reference.exec("B");

    assert!(result.tree.is_none());
    assert_eq!(result.pattern_error, Some(unknown("value", "value")));
}

#[test]
fn test_no_match() {
    let value = value_pattern();
    let mut reference = the_reference(&value);

    assert!(!reference.test("B"));
}

#[test]
fn resolves_through_a_context_that_does_not_contain_the_target() {
    // The inner context has no "greeting", so the lookup goes on to the
    // outer one. Driven through `parse`, as in v12: contexts need no exec.
    let inner = context("unrelated", seq("seq", vec![reference("greeting")]), vec![]);
    let mut outer = context("outer", inner, vec![lit("greeting", "Hi")]);
    let mut cursor = Cursor::new("Hi");

    let node = outer.parse(&mut cursor).unwrap();

    let tree = cursor.into_tree(node);
    assert_eq!(tree.root().value(), "Hi");
    // v12's outer context cloned the inner one, which renamed its child.
    assert_eq!(outline(tree.root()), "unrelated@0..2[greeting@0..2]");
}

// ─── Recursion ──────────────────────────────────────────────────────────

/// `expr = "(" expr ")" | "x"`: a reference inside a sequence inside a
/// choice.
fn parens() -> Pattern {
    choice(
        "expr",
        vec![
            seq(
                "group",
                vec![lit("open", "("), reference("expr"), lit("close", ")")],
            ),
            lit("x", "x"),
        ],
    )
}

#[test]
fn a_recursive_grammar_parses_nested_input() {
    let mut expr = parens();

    assert_eq!(
        exec_outline(&mut expr, "((x))"),
        outlined(
            "group@0..5[open@0..1 group@1..4[open@1..2 x@2..3 close@3..4] close@4..5]",
            "-",
            "open@2..2"
        )
    );
    assert_eq!(
        exec_outline(&mut expr, "((x)"),
        outlined("-", "expr@0..4", "group@0..4")
    );
    assert_eq!(
        exec_outline(&mut expr, "((y))"),
        outlined("-", "expr@0..5", "open@2..2")
    );
}

#[test]
fn each_recursion_level_parses_its_own_instance() {
    // Every level's `group` sequence keeps its children in scratch space on
    // itself. If the levels shared one instance, the inner parse would
    // overwrite the outer one's children mid-parse and the outer nodes
    // would come out wrong. Six levels, each with the right three children.
    let mut expr = parens();

    let (tree, _, _) = exec_outline(&mut expr, "((((((x))))))");

    assert_eq!(
        tree,
        "group@0..13[open@0..1 group@1..12[open@1..2 group@2..11[open@2..3 group@3..10[open@3..4 \
         group@4..9[open@4..5 group@5..8[open@5..6 x@6..7 close@7..8] close@8..9] close@9..10] \
         close@10..11] close@11..12] close@12..13]"
    );
}

#[test]
fn resolved_instances_are_reused_by_later_parses() {
    // Instances stay resolved between execs (v12 keeps `_pattern`), so a
    // shallower and then a deeper input both parse right on one pattern.
    let mut expr = parens();

    assert!(expr.test("((x))"));
    assert!(expr.test("x"));
    assert!(expr.test("(((x)))"));
    assert!(!expr.test("((x)"));
    assert_eq!(
        exec_outline(&mut expr, "(x)").0,
        "group@0..3[open@0..1 x@1..2 close@2..3]"
    );
}

/// `a = a "b" | "b"`, built by hand. In v12 a sequence that starts with a
/// child named like the choice becomes a postfix operator (the Pratt mode,
/// a later milestone); the expected values come from v12 with the
/// reference renamed, which keeps it an ordered choice like this one.
fn left_recursive() -> Pattern {
    choice(
        "a",
        vec![
            seq("ab", vec![reference("a"), lit("b", "b")]),
            lit("b", "b"),
        ],
    )
}

#[test]
fn left_recursion_terminates_with_v12s_results() {
    let mut a = left_recursive();

    assert_eq!(
        exec_outline(&mut a, "b"),
        outlined("b@0..1", "-", "ab@0..1")
    );
    assert_eq!(
        exec_outline(&mut a, "bb"),
        outlined("ab@0..2[b@0..1 b@1..2]", "-", "-")
    );
    assert_eq!(
        exec_outline(&mut a, "bbb"),
        outlined("-", "a@2..3", "a@2..3")
    );
    assert_eq!(exec_outline(&mut a, "c"), outlined("-", "a@0..1", "b@0..0"));
    assert_eq!(
        exec_outline(&mut a, "bc"),
        outlined("-", "a@1..2", "b@1..1")
    );
}

#[test]
fn the_references_own_guard_stops_left_recursion_through_a_sequence() {
    // `a = r? "x"` with `r` → `a`: no choice to skip anything, so the
    // reference's guard does it. The target is let in once at a position
    // (the inner level), and the second time it fails with its own error:
    // that error, `r@0..0`, is v12's furthest error too.
    let mut a = seq("a", vec![opt("o", reference_to("r", "a")), lit("x", "x")]);

    assert_eq!(
        exec_outline(&mut a, "xx"),
        outlined("a@0..2[r@0..1[x@0..1] x@1..2]", "-", "r@0..0")
    );
    assert_eq!(exec_outline(&mut a, "x"), outlined("-", "a@0..1", "a@0..1"));
    assert_eq!(
        exec_outline(&mut a, "xxx"),
        outlined("-", "a@2..3", "r@0..0")
    );
}

#[test]
fn mutually_recursive_choices_terminate() {
    // `a = b | "c"`, `b = a | "d"`. The choice skips an alternative already
    // parsing at the same spot higher up, as v12's does.
    let a = choice("a", vec![reference("b"), lit("c", "c")]);
    let b = choice("b", vec![reference("a"), lit("d", "d")]);
    let mut root = context("root", a.clone(), vec![a, b]);

    assert_eq!(exec_outline(&mut root, "c"), outlined("c@0..1", "-", "-"));
    assert_eq!(
        exec_outline(&mut root, "d"),
        outlined("d@0..1", "-", "c@0..0")
    );
    assert_eq!(
        exec_outline(&mut root, "e"),
        outlined("-", "a@0..1", "c@0..0")
    );
}

#[test]
fn recursion_through_a_repeat_and_a_context() {
    let list = || {
        seq(
            "list",
            vec![
                lit("open", "["),
                opt(
                    "maybe",
                    rep("items", reference("item"), divided(lit("comma", ","))),
                ),
                lit("close", "]"),
            ],
        )
    };
    let item = choice("item", vec![re("n", "[0-9]+"), reference("list")]);
    let mut root = context("c", list(), vec![item, list()]);

    assert_eq!(
        exec_outline(&mut root, "[1,[2,[]],3]").0,
        "list@0..12[open@0..1 items@1..11[n@1..2 comma@2..3 list@3..9[open@3..4 items@4..8[n@4..5 \
         comma@5..6 list@6..8[open@6..7 close@7..8]] close@8..9] comma@9..10 n@10..11] close@11..12]"
    );
    assert_eq!(
        exec_outline(&mut root, "[1,[2,]"),
        outlined("-", "list@0..7", "list@0..7")
    );
}

// ─── Resolution ─────────────────────────────────────────────────────────

#[test]
fn an_unknown_name_is_reported_by_exec_without_parsing() {
    // v12 throws here too: its choice resolves references while it builds.
    let mut pattern = choice("c", vec![lit("a", "a"), reference("missing")]);

    let result = pattern.exec("a");

    assert!(result.tree.is_none());
    assert!(result.error.is_none());
    assert!(result.furthest_error.is_none());
    assert_eq!(result.pattern_error, Some(unknown("missing", "missing")));
    // Every time, not just the first (v12's second exec of a half-built
    // choice quietly stops throwing).
    assert_eq!(
        pattern.exec("a").pattern_error,
        Some(unknown("missing", "missing"))
    );
    assert!(!pattern.test("a"));
}

#[test]
fn an_unknown_name_is_reported_even_on_empty_text() {
    let mut pattern = seq("s", vec![reference_to("r", "nowhere")]);

    assert_eq!(
        pattern.exec("").pattern_error,
        Some(unknown("r", "nowhere"))
    );
}

#[test]
fn check_reports_unknown_names_up_front() {
    let pattern = seq("s", vec![lit("a", "a"), reference_to("r", "word")]);

    assert_eq!(pattern.check(), Err(unknown("r", "word")));
    assert_eq!(parens().check(), Ok(()));
    assert_eq!(value_pattern().check(), Ok(()));
}

#[test]
fn check_follows_references_into_what_they_resolve_to() {
    // `item` is fine at the top, but the `list` it resolves to names
    // `missing`. The parse would only meet that one level down.
    let item = choice("item", vec![lit("n", "1"), reference("list")]);
    let list = seq("list", vec![lit("open", "["), reference("missing")]);
    let root = context("c", reference("item"), vec![item, list]);

    assert_eq!(root.check(), Err(unknown("missing", "missing")));
}

#[test]
fn the_unknown_name_error_explains_the_fix() {
    let message = unknown("r", "word").to_string();

    assert!(message.contains("`r`"), "{message}");
    assert!(message.contains("`word`"), "{message}");
    assert!(message.contains("Context"), "{message}");
}

#[test]
fn a_reference_never_resolves_to_another_reference() {
    // The root's tree has a reference named "x"; a reference to "x" skips
    // it, finds nothing else, and so names nothing (v12 `_isValidPattern`).
    let mut pattern = seq("s", vec![reference_to("x", "y"), reference("x")]);

    assert_eq!(pattern.exec("a").pattern_error, Some(unknown("x", "y")));
    assert_eq!(pattern.check(), Err(unknown("x", "y")));
}

#[test]
fn a_context_that_only_wraps_a_reference_is_skipped() {
    // The inner scope's `x` is a context around a reference: not a valid
    // target (v12 `_isValidPattern`), so the lookup goes on to the outer
    // scope and finds the literal there. Resolving to the wrapper would
    // match "y" instead.
    let mut root = context(
        "outer",
        context(
            "inner",
            seq("s", vec![reference("x")]),
            vec![context("x", reference("y"), vec![lit("y", "y")])],
        ),
        vec![lit("x", "q"), lit("y", "y")],
    );

    assert_eq!(exec_outline(&mut root, "q").0, "inner@0..1[x@0..1]");
    assert_eq!(
        exec_outline(&mut root, "y"),
        outlined("-", "inner@0..1", "x@0..0")
    );
}

#[test]
fn the_implicit_scope_takes_the_first_match_depth_first() {
    // Two patterns named "t": the one nested deeper inside the first child
    // comes first in v12's search (children before their parent, in order).
    let mut pattern = seq(
        "s",
        vec![
            seq("wrap", vec![lit("t", "a")]),
            lit("t", "b"),
            opt("o", reference("t")),
        ],
    );

    assert!(pattern.test("aba"));
    assert!(!pattern.test("abb"));
}

#[test]
fn a_lookahead_at_the_end_resolves_references_too() {
    // `!a` at the very end probes on an empty cursor, which keeps the scopes.
    let mut pattern = seq("s", vec![lit("a", "a"), not("n", reference("a"))]);

    assert_eq!(exec_outline(&mut pattern, "a").0, "s@0..1[a@0..1]");
    assert_eq!(
        exec_outline(&mut pattern, "aa"),
        outlined("-", "s@0..2", "n@1..1")
    );
}

#[test]
fn a_lookahead_probe_at_the_end_sees_the_scopes() {
    // `!o` at the end, where `o` is an optional: through the reference it
    // matches the empty rest, so the lookahead fails (as in v12). A probe
    // that couldn't resolve `o` would call that a failure, and the
    // lookahead would wrongly hold.
    let mut pattern = seq(
        "s",
        vec![
            lit("a", "a"),
            opt("o", lit("b", "b")),
            not("n", reference("o")),
        ],
    );

    assert_eq!(
        exec_outline(&mut pattern, "a"),
        outlined("-", "s@0..1", "n@1..1")
    );
    assert_eq!(
        exec_outline(&mut pattern, "ab"),
        outlined("-", "s@0..2", "n@2..2")
    );
}

#[test]
fn a_pattern_moved_into_a_bigger_one_resolves_in_its_new_root() {
    // `p` is exec'd alone first, which readies its references against its
    // own tree ("t" = "a"). Moved into `r`, whose tree finds "t" = "b"
    // first, its still-unresolved reference must look in `r`, as v12's
    // clone does.
    let mut p = seq("p", vec![lit("t", "a"), opt("o", reference("t"))]);
    assert!(p.test("a"));
    let mut r = seq("r", vec![lit("t", "b"), p]);

    assert!(r.test("bab"));
    assert!(!r.test("baa"));
}

#[test]
fn a_renamed_root_is_checked_again() {
    // A renamed root is a different grammar: renamed `other`, it no longer
    // defines `expr`, so its inner reference names nothing (v12 throws
    // "Couldn't find 'expr'"). The renamed clone must not inherit the
    // original's readied scope, which would let it through.
    let mut expr = parens();
    assert!(expr.test("(x)"));

    let mut renamed = expr.renamed("other");

    assert_eq!(
        renamed.exec("((x))").pattern_error,
        Some(unknown("expr", "expr"))
    );
    assert!(expr.test("((x))"));
}

#[test]
fn references_and_their_clones_keep_their_identity() {
    let original = Reference::to("r", "target");
    let clone = original.clone();

    assert_eq!(clone.target_name(), "target");
    let original: Pattern = original.into();
    assert_eq!(Pattern::from(clone).id(), original.id());
    assert_eq!(original.renamed("s").id(), original.id());
}

#[test]
fn the_implicit_scope_searches_a_patterns_children_before_the_pattern() {
    // A sequence named "t" holds a literal named "t". v12's search reaches
    // the literal first, so `r` matches "x", not "xy".
    let mut pattern = seq(
        "root",
        vec![
            seq("t", vec![lit("t", "x"), lit("y", "y")]),
            reference_to("r", "t"),
        ],
    );

    assert_eq!(
        exec_outline(&mut pattern, "xyx").0,
        "root@0..3[t@0..2[t@0..1 y@1..2] r@2..3]"
    );
    assert_eq!(
        exec_outline(&mut pattern, "xyxy"),
        outlined("-", "root@3..4", "root@3..4")
    );
}

#[test]
fn a_clone_of_a_resolved_reference_keeps_its_master() {
    // v12 `clone` copies the resolved pattern into `_cachedPattern`. Taken
    // out of its tree, the clone has nothing to search, yet still parses
    // the literal it resolved to.
    let mut s = seq("s", vec![lit("t", "a"), opt("o", reference("t"))]);
    assert!(s.test("aa"));
    let Pattern::Sequence(sequence) = &s else {
        unreachable!()
    };
    let Pattern::Optional(optional) = &sequence.children()[1] else {
        unreachable!()
    };
    let mut clone = optional.child().clone();

    assert_eq!(exec_outline(&mut clone, "a").0, "t@0..1");
    assert_eq!(
        exec_outline(&mut clone, "b"),
        outlined("-", "t@0..1", "t@0..0")
    );
}

#[test]
fn an_unknown_name_met_while_parsing_halts_the_parse() {
    // v12 throws as soon as the parse reaches the name, so no later
    // alternative gets a turn. Driving `parse` directly skips `exec`'s
    // check; the halt is that throw as data. Without it, "t" would match.
    let mut pattern = choice("c", vec![reference("missing"), lit("t", "t")]);
    let mut cursor = Cursor::new("t");

    assert!(pattern.parse(&mut cursor).is_none());

    assert!(cursor.halted());
    assert_eq!(cursor.pattern_error(), Some(&unknown("missing", "missing")));
}

/// `k` contexts that all refer to one another: v12's lazy lookup handles
/// them; a check that told apart every order of nested contexts would
/// grow with k! (and overflowed the stack at k = 7).
fn mutual_contexts(k: usize) -> Pattern {
    let names: Vec<String> = (0..k).map(|i| format!("c{i}")).collect();
    let contexts = (0..k)
        .map(|i| {
            let mut alternatives: Vec<Pattern> = (0..k)
                .filter(|&j| j != i)
                .map(|j| {
                    seq(
                        &format!("s{i}_{j}"),
                        vec![lit("l", &format!("<{j}")), reference(&names[j])],
                    )
                })
                .collect();
            alternatives.push(lit("end", "."));
            context(&names[i], choice(&format!("ch{i}"), alternatives), vec![])
        })
        .collect();
    context("root", reference("c0"), contexts)
}

#[test]
fn check_handles_many_mutually_referring_contexts() {
    // The same answers as v12 (`true true false`, from its run of this
    // grammar at k = 7 and k = 12).
    let mut root = mutual_contexts(7);

    assert_eq!(root.check(), Ok(()));
    assert!(root.test("<1<2."));
    assert!(root.test("<1<2<3<1<4."));
    assert!(!root.test("<9"));
}

#[test]
fn check_walks_long_reference_chains_without_recursing() {
    // `a0 -> a1 -> … -> a4999 -> missing`: finding `missing` means
    // following five thousand references. A check that recursed per
    // reference would overflow a test thread's stack before getting there.
    let chain = (0..5000)
        .map(|i| {
            let next = if i == 4999 {
                "missing".to_string()
            } else {
                format!("a{}", i + 1)
            };
            seq(&format!("a{i}"), vec![lit("x", "x"), reference(&next)])
        })
        .collect();
    let root = context("root", reference("a0"), chain);

    assert_eq!(root.check(), Err(unknown("missing", "missing")));
}

#[test]
fn check_reports_the_first_unknown_name_in_parse_order() {
    // Two references, each to a pattern that names something unknown. The
    // one the parse would reach first is the one reported.
    let first = seq("first", vec![lit("a", "a"), reference("nope1")]);
    let second = seq("second", vec![lit("b", "b"), reference("nope2")]);
    let root = context(
        "root",
        seq("s", vec![reference("first"), reference("second")]),
        vec![first, second],
    );

    assert_eq!(root.check(), Err(unknown("nope1", "nope1")));
}

// ─── Where an implicit-scope target resolves its own names ──────────────
//
// v12's implicit scope (`findPattern(root)`) returns the LIVE pattern in the
// root's tree, and cloning it keeps what its references resolved to there.
// So once the definition has parsed, a reference from anywhere sees the
// names of the contexts around the definition. cpat-rs gives an implicit
// target that home always (`scope::Found::home`). Every expectation below
// was produced by v12 on the same pattern and input.

/// `t` refers to `u`, defined only in the context `k` around `t`; `rt`
/// reaches `t` from outside `k`.
fn defined_in_a_context(t: Pattern) -> Pattern {
    seq(
        "root",
        vec![
            context("k", seq("w", vec![t]), vec![lit("u", "K")]),
            lit("sep", ";"),
            reference_to("rt", "t"),
        ],
    )
}

fn t_refers_to_u() -> Pattern {
    seq("t", vec![reference("u")])
}

#[test]
fn an_implicit_target_resolves_names_where_it_is_defined() {
    // Resolved from where `rt` sits, `u` names nothing: this used to be
    // `UnknownReference` on every input.
    let mut pattern = defined_in_a_context(t_refers_to_u());

    assert_eq!(
        exec_outline(&mut pattern, "K;K"),
        outlined(
            "root@0..3[k@0..1[t@0..1[u@0..1]] sep@1..2 rt@2..3[u@2..3]]",
            "-",
            "-"
        )
    );
    assert_eq!(
        exec_outline(&mut pattern, "K;X"),
        outlined("-", "root@0..3", "u@2..2")
    );
    assert_eq!(pattern.check(), Ok(()));
}

#[test]
fn an_implicit_target_prefers_its_own_context_over_the_references() {
    // The outer context `R` defines `u` too, and `rt` sits inside it. `t`
    // still means `k`'s `u`.
    let mut pattern = context(
        "R",
        defined_in_a_context(t_refers_to_u()),
        vec![lit("u", "Z")],
    );

    assert_eq!(
        exec_outline(&mut pattern, "K;K"),
        outlined(
            "root@0..3[k@0..1[t@0..1[u@0..1]] sep@1..2 rt@2..3[u@2..3]]",
            "-",
            "-"
        )
    );
    assert_eq!(
        exec_outline(&mut pattern, "K;Z"),
        outlined("-", "root@0..3", "u@2..2")
    );
}

#[test]
fn an_implicit_choice_target_resolves_where_it_is_defined() {
    let t = choice("t", vec![reference("u"), lit("q", "q")]);
    let mut pattern = context("R", defined_in_a_context(t), vec![lit("u", "Z")]);

    assert_eq!(
        exec_outline(&mut pattern, "K;K"),
        outlined("root@0..3[k@0..1[u@0..1] sep@1..2 u@2..3]", "-", "-")
    );
    assert_eq!(
        exec_outline(&mut pattern, "K;Z"),
        outlined("-", "root@0..3", "u@2..2")
    );
}

#[test]
fn an_implicit_target_recurses_at_home() {
    // `e = "(" e ")" | u` inside `k`, reached from outside by `re`: every
    // level, however deep, resolves `u` in `k`.
    let e = choice(
        "e",
        vec![
            seq("p", vec![lit("l", "("), reference("e"), lit("r", ")")]),
            reference("u"),
        ],
    );
    let mut pattern = seq(
        "root",
        vec![
            context("k", seq("w", vec![e]), vec![lit("u", "K")]),
            lit("sep", ";"),
            reference_to("re", "e"),
        ],
    );

    assert_eq!(
        exec_outline(&mut pattern, "K;K"),
        outlined("root@0..3[k@0..1[u@0..1] sep@1..2 u@2..3]", "-", "l@2..2")
    );
    assert_eq!(
        exec_outline(&mut pattern, "(K);((K))"),
        outlined(
            "root@0..9[k@0..3[p@0..3[l@0..1 u@1..2 r@2..3]] sep@3..4 \
             p@4..9[l@4..5 p@5..8[l@5..6 u@6..7 r@7..8] r@8..9]]",
            "-",
            "l@6..6"
        )
    );
    assert_eq!(
        exec_outline(&mut pattern, "K;((X))"),
        outlined("-", "root@0..7", "l@4..4")
    );
}

#[test]
fn a_top_level_implicit_target_ignores_a_context_around_the_reference() {
    // `t` sits in no context, so it resolves `u` in the root's tree ("K"),
    // even though `rt` sits in a context where `u` is "Z".
    let mut pattern = seq(
        "root",
        vec![
            t_refers_to_u(),
            lit("u", "K"),
            lit("sep", ";"),
            context("C", reference_to("rt", "t"), vec![lit("u", "Z")]),
        ],
    );

    assert_eq!(
        exec_outline(&mut pattern, "KK;K"),
        outlined(
            "root@0..4[t@0..1[u@0..1] u@1..2 sep@2..3 C@3..4[u@3..4]]",
            "-",
            "-"
        )
    );
    assert_eq!(
        exec_outline(&mut pattern, "KK;Z"),
        outlined("-", "root@0..4", "u@3..3")
    );
}

#[test]
fn deviation_a_reference_parsed_before_the_definition_still_resolves_at_home() {
    // DEVIATION (DESIGN §11). Here `rt` parses before `t` ever has. v12
    // then clones a `t` whose `u` is still unresolved, so `u` resolves
    // from `rt`'s place: the outer `R` ("Z" matches) or, without `R`,
    // nothing at all (v12 throws "Couldn't find 'u'"). Which one v12 does
    // depends on what earlier parses reached; cpat-rs always resolves at
    // home, so "K" matches in both.
    let inner = || {
        seq(
            "root",
            vec![
                reference_to("rt", "t"),
                lit("sep", ";"),
                context("k", seq("w", vec![t_refers_to_u()]), vec![lit("u", "K")]),
            ],
        )
    };
    let mut alone = inner();
    let mut wrapped = context("R", inner(), vec![lit("u", "Z")]);

    let expected = outlined(
        "root@0..3[rt@0..1[u@0..1] sep@1..2 k@2..3[t@2..3[u@2..3]]]",
        "-",
        "-",
    );
    assert_eq!(exec_outline(&mut alone, "K;K"), expected);
    assert_eq!(exec_outline(&mut wrapped, "K;K"), expected);
    assert_eq!(
        exec_outline(&mut wrapped, "Z;K"),
        outlined("-", "root@0..3", "u@0..0")
    );
}

/// `t = ("a" [cut] u) | "ab"`: a choice target whose sequence alternative
/// refers to `u`, defined only in `k` around `t`.
fn choice_with_a_sequence_alternative(with_cut: bool) -> Pattern {
    let s = if with_cut {
        seq("s", vec![lit("a", "a"), cut(), reference("u")])
    } else {
        seq("s", vec![lit("a", "a"), reference("u")])
    };
    choice("t", vec![s, lit("ab", "ab")])
}

#[test]
fn deviation_a_reference_wrapped_inside_a_choice_target_still_resolves_at_home() {
    // DEVIATION (DESIGN §11). v12's `Expression.clone` rebuilds the choice
    // from its never-parsed original alternatives, so a reference wrapped
    // in a sequence, optional or repeat beneath the choice loses its home
    // even when the definition parsed first, and resolves from `rt`'s
    // place: without `R`, v12 throws "Couldn't find 'u' pattern within
    // tree." on every input below; with `R`, it matches "Z" and rejects
    // "K". (A reference that IS an alternative keeps its home in v12: see
    // `an_implicit_choice_target_resolves_where_it_is_defined`.) cpat-rs
    // resolves at home whatever wraps the reference.
    let mut plain = defined_in_a_context(choice_with_a_sequence_alternative(false));
    assert_eq!(
        exec_outline(&mut plain, "aK;aK"),
        outlined(
            "root@0..5[k@0..2[s@0..2[a@0..1 u@1..2]] sep@2..3 s@3..5[a@3..4 u@4..5]]",
            "-",
            "-"
        )
    );
    assert_eq!(
        exec_outline(&mut plain, "ab;aK"),
        outlined(
            "root@0..5[k@0..2[ab@0..2] sep@2..3 s@3..5[a@3..4 u@4..5]]",
            "-",
            "u@1..1"
        )
    );
    assert_eq!(plain.check(), Ok(()));

    let mut with_cut = defined_in_a_context(choice_with_a_sequence_alternative(true));
    assert_eq!(
        exec_outline(&mut with_cut, "aK;aK"),
        outlined(
            "root@0..5[k@0..2[s@0..2[a@0..1 u@1..2]] sep@2..3 s@3..5[a@3..4 u@4..5]]",
            "-",
            "-"
        )
    );
    // The cut after "a" commits `s`, so "ab" is not tried.
    assert_eq!(
        exec_outline(&mut with_cut, "aK;ab"),
        outlined("-", "root@0..5", "u@4..4")
    );

    let mut wrapped = context(
        "R",
        defined_in_a_context(choice_with_a_sequence_alternative(false)),
        vec![lit("u", "Z")],
    );
    assert_eq!(
        exec_outline(&mut wrapped, "aK;aK"),
        outlined(
            "root@0..5[k@0..2[s@0..2[a@0..1 u@1..2]] sep@2..3 s@3..5[a@3..4 u@4..5]]",
            "-",
            "-"
        )
    );
    assert_eq!(
        exec_outline(&mut wrapped, "aK;aZ"),
        outlined("-", "root@0..5", "u@4..4")
    );

    let optional = choice("t", vec![opt("o", reference("u")), lit("ab", "ab")]);
    let mut wrapped = context("R", defined_in_a_context(optional), vec![lit("u", "Z")]);
    assert_eq!(
        exec_outline(&mut wrapped, "K;K"),
        outlined("root@0..3[k@0..1[u@0..1] sep@1..2 u@2..3]", "-", "-")
    );
    assert_eq!(
        exec_outline(&mut wrapped, "K;Z"),
        outlined("-", "root@0..3", "u@2..2")
    );
}

#[test]
fn a_choice_alternative_inside_a_sequence_target_resolves_at_home() {
    // Agrees with v12: the reference is itself an alternative of the
    // nested choice, which v12's rebuilt choice still binds at home.
    let t = seq(
        "t",
        vec![
            lit("x", "x"),
            choice("c", vec![reference("u"), lit("ab", "ab")]),
        ],
    );
    let mut pattern = defined_in_a_context(t);

    assert_eq!(
        exec_outline(&mut pattern, "xK;xK"),
        outlined(
            "root@0..5[k@0..2[t@0..2[x@0..1 u@1..2]] sep@2..3 rt@3..5[x@3..4 u@4..5]]",
            "-",
            "-"
        )
    );
}
