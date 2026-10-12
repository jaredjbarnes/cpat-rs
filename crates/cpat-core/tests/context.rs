//! Ports of v12 `Context.test.ts`, plus naming and lookup cases checked
//! against v12 on the same inputs.
//!
//! Deferred: 'With Grammar' (the grammar milestone), and 'isEqual…',
//! 'find locates…', 'standalone introspection…', 'introspection delegates…',
//! 'getTokens delegates…' and 'getPatterns delegates…' (the introspection
//! API).

mod common;

use common::*;
use cpat_core::{Context, Pattern};

fn names_pattern() -> Pattern {
    seq(
        "names",
        vec![lit("john", "John"), reference("space"), lit("jane", "Jane")],
    )
}

#[test]
fn basic() {
    let names = names_pattern();
    let mut context = context("context", names.clone(), vec![lit("space", " "), names]);

    let result = context.exec("John Jane");

    assert_eq!(result.tree().unwrap().root().value(), "John Jane");
}

#[test]
fn test_returns_boolean_match_through_context() {
    let mut context = context("context", names_pattern(), vec![lit("space", " ")]);

    assert!(context.test("John Jane"));
    assert!(!context.test("John Bob"));
}

#[test]
fn get_patterns_within_context_lists_sibling_patterns() {
    let context = Context::new(
        "context",
        names_pattern(),
        vec![lit("space", " "), lit("jane", "Jane")],
    );

    let available = context.patterns_within_context();

    assert_eq!(available.len(), 2);
    assert_eq!(available["space"].name(), "space");
    assert_eq!(available["jane"].name(), "jane");
}

#[test]
fn get_patterns_within_context_returns_a_copy() {
    // A fresh map per call, listing the same definitions. The map borrows
    // the patterns read-only, so callers can't reach the context's state.
    let context = Context::new("ctx", lit("x", "x"), vec![lit("space", " ")]);

    let a = context.patterns_within_context();
    let b = context.patterns_within_context();

    assert_eq!(a.len(), b.len());
    assert!(a.iter().all(|(name, p)| b[name].id() == p.id()));
}

// ─── Behavior beyond the ported tests, each checked against v12 ─────────

#[test]
fn the_result_is_the_childs_node_and_the_summary_names_the_child() {
    // v12's `Context.exec` runs its child's exec, so a failed parse's
    // summary error is the child's.
    let mut context = context("context", names_pattern(), vec![lit("space", " ")]);

    assert_eq!(
        exec_outline(&mut context, "John Jane"),
        outlined("names@0..9[john@0..4 space@4..5 jane@5..9]", "-", "-")
    );
    assert_eq!(
        exec_outline(&mut context, "John Bob"),
        outlined("-", "names@0..8", "jane@5..5")
    );
    assert_eq!(
        exec_outline(&mut context, "John"),
        outlined("-", "names@0..4", "names@0..4")
    );
}

#[test]
fn a_context_inside_a_composite_names_its_childs_nodes_after_itself() {
    // v12's composites clone their children, and a cloned context renames
    // its child to its own name. Rust composites take their children over
    // and apply the same rename (`Pattern::adopted`).
    let mut pattern = seq(
        "s",
        vec![context("c", lit("a", "a"), vec![]), lit("b", "b")],
    );

    assert_eq!(exec_outline(&mut pattern, "ab").0, "s@0..2[c@0..1 b@1..2]");
}

#[test]
fn a_context_run_directly_keeps_its_childs_name() {
    let mut context = context("c", lit("a", "a"), vec![]);

    assert_eq!(exec_outline(&mut context, "a").0, "a@0..1");
}

#[test]
fn a_reference_to_the_contexts_own_name_finds_the_context() {
    // v12 `getPatternWithinContext`: the context answers to its name with
    // itself. The resolved copy is renamed after the reference, child too.
    let mut list = context(
        "list",
        seq("item", vec![lit("a", "a"), opt("rest", reference("list"))]),
        vec![],
    );

    assert_eq!(
        exec_outline(&mut list, "aaa").0,
        "item@0..3[a@0..1 list@1..3[a@1..2 list@2..3[a@2..3]]]"
    );
    assert_eq!(
        exec_outline(&mut list, "ab"),
        outlined("-", "item@1..2", "a@1..1")
    );
}

#[test]
fn a_renamed_reference_to_the_context_names_the_nodes_after_itself() {
    let mut list = context(
        "list",
        seq(
            "item",
            vec![lit("a", "a"), opt("rest", reference_to("more", "list"))],
        ),
        vec![],
    );

    assert_eq!(
        exec_outline(&mut list, "aaa").0,
        "item@0..3[a@0..1 more@1..3[a@1..2 more@2..3[a@2..3]]]"
    );
}

#[test]
fn a_context_nested_in_another_contexts_list_is_not_in_the_root_tree() {
    // `word` is only a definition inside `holder`'s list, not a pattern in
    // the tree, so nothing outside `holder` can find it (v12 throws).
    let inner = context("word", re("w", "[a-z]+"), vec![]);
    let pattern = seq(
        "s",
        vec![
            reference_to("r", "word"),
            lit("dot", "."),
            context("holder", lit("z", "z"), vec![inner]),
        ],
    );

    assert!(pattern.check().is_err());
}

#[test]
fn later_definitions_of_a_name_win() {
    // v12 fills a record, so the last pattern with a name is the one kept.
    let mut context = context(
        "c",
        seq("s", vec![reference("t")]),
        vec![lit("t", "first"), lit("t", "second")],
    );

    assert!(context.test("second"));
    assert!(!context.test("first"));
}

#[test]
fn the_innermost_context_wins() {
    let mut pattern = context(
        "outer",
        context(
            "inner",
            seq("s", vec![reference("t")]),
            vec![lit("t", "in")],
        ),
        vec![lit("t", "out")],
    );

    assert!(pattern.test("in"));
    assert!(!pattern.test("out"));
}

#[test]
fn a_renamed_context_still_answers_to_its_original_name() {
    // v12 `clone("d")` keeps `_referencePatternName`, so the reference to
    // "c" inside still finds the context. Nothing else in the tree is named
    // "c" (the rename reached the child too), so only that answer works.
    let mut renamed = context(
        "c",
        seq("s", vec![lit("a", "a"), opt("o", reference("c"))]),
        vec![],
    )
    .renamed("d");

    assert_eq!(exec_outline(&mut renamed, "a").0, "d@0..1[a@0..1]");
    assert_eq!(
        exec_outline(&mut renamed, "aa").0,
        "d@0..2[a@0..1 c@1..2[a@1..2]]"
    );
}

#[test]
fn a_context_resolved_by_a_reference_answers_to_the_references_name() {
    // `e` resolves the context `c` and renames it `e` (v12 `clone("e")`).
    // Inside, a reference to "e" is answered by that context under its
    // current name: no definition anywhere is called "e".
    let mut root = context(
        "root",
        reference_to("e", "c"),
        vec![context(
            "c",
            seq("s", vec![lit("a", "a"), opt("o", reference("e"))]),
            vec![],
        )],
    );

    assert_eq!(exec_outline(&mut root, "a").0, "e@0..1[a@0..1]");
    assert_eq!(
        exec_outline(&mut root, "aaa").0,
        "e@0..3[a@0..1 e@1..3[a@1..2 e@2..3[a@2..3]]]"
    );
    assert_eq!(
        exec_outline(&mut root, "b"),
        outlined("-", "e@0..1", "a@0..0")
    );
}
