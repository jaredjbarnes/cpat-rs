//! Limits on one parse (DESIGN §11): input nested deeper than the parser
//! allows stops the parse as data, never as a stack overflow. v12 throws a
//! `RangeError` from the JS engine instead.

mod common;

use common::*;
use cpat_core::{Cursor, DEFAULT_MAX_DEPTH, Limits, Node, NodeKind, Pattern, TooDeep};

/// `expr = "(" expr ")" | "x"`: three composites (choice, sequence,
/// reference) per level of parentheses.
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

fn nested(levels: usize) -> String {
    format!("{}x{}", "(".repeat(levels), ")".repeat(levels))
}

fn limited(max_depth: usize) -> Limits {
    Limits { max_depth }
}

#[test]
fn input_nested_too_deep_stops_the_parse_as_data() {
    // 100 000 levels would overflow any stack (natively an abort, which no
    // test could survive). Here the parse stops at the limit and says so.
    let mut expr = parens();
    let text = nested(100_000);

    let result = expr.exec(&text);

    assert!(result.tree.is_none());
    let too_deep = result.too_deep.expect("stopped at the depth limit");
    assert_eq!(too_deep.max_depth, DEFAULT_MAX_DEPTH);
    assert!(result.pattern_error.is_none());
}

#[test]
fn the_pattern_still_works_after_a_parse_stopped() {
    // In wasm a stack overflow left the module broken for every later call.
    // A halt is an ordinary return: the same pattern parses again.
    let mut expr = parens();
    assert!(expr.exec(&nested(100_000)).too_deep.is_some());

    assert_eq!(
        exec_outline(&mut expr, "((x))").0,
        "group@0..5[open@0..1 group@1..4[open@1..2 x@2..3 close@3..4] close@4..5]"
    );
}

#[test]
fn nesting_within_the_default_limit_parses() {
    let mut expr = parens();

    assert!(expr.test(&nested(300)));
}

#[test]
fn the_limit_counts_composites_exactly() {
    // `((x))` runs choice, sequence, reference twice, then the innermost
    // choice, which tries its `group` sequence at the `x` before it finds
    // the literal: eight composites at once.
    let mut expr = parens();

    assert!(expr.exec_with("((x))", limited(8)).is_match());

    let result = expr.exec_with("((x))", limited(7));
    assert!(result.tree.is_none());
    // The eighth, that innermost `group`, would have begun at 2.
    assert_eq!(
        result.too_deep,
        Some(TooDeep {
            max_depth: 7,
            pos: 2
        })
    );
    assert_eq!(error_outline(result.error), "group@2..2");
}

#[test]
fn a_halted_parse_tries_no_other_alternative() {
    // The regex alternative would match the whole text, but the parse had
    // already stopped: the answer is "too deep", not a tree from a fallback.
    let mut pattern = choice(
        "expr",
        vec![
            seq(
                "group",
                vec![lit("open", "("), reference("expr"), lit("close", ")")],
            ),
            re("loose", "[()x]+"),
        ],
    );

    assert!(pattern.test("((x))"));

    let result = pattern.exec_with("((x))", limited(4));
    assert!(result.tree.is_none());
    assert!(result.too_deep.is_some());
}

#[test]
fn a_lookahead_probe_that_goes_too_deep_stops_the_parse() {
    // `!c` at the end of the text runs `c` on a probe cursor, which inherits
    // the depth: sequence, lookahead, choice is three composites. The probe
    // halting must halt the real parse, not read as "c didn't match".
    let mut pattern = seq(
        "s",
        vec![lit("a", "a"), not("n", choice("c", vec![lit("b", "b")]))],
    );

    assert!(pattern.exec_with("a", limited(3)).is_match());

    let result = pattern.exec_with("a", limited(2));
    assert!(result.tree.is_none());
    assert_eq!(
        result.too_deep,
        Some(TooDeep {
            max_depth: 2,
            pos: 1
        })
    );
}

#[test]
fn a_cursor_driven_directly_reports_the_halt() {
    // Three nested sequences, two allowed: the innermost one, at 1, halts.
    let mut pattern = seq(
        "a",
        vec![lit("x", "x"), seq("b", vec![seq("c", vec![lit("y", "y")])])],
    );
    let mut cursor = Cursor::with_limits("xy", limited(2));

    assert!(pattern.parse(&mut cursor).is_none());

    assert!(cursor.halted());
    assert_eq!(
        cursor.too_deep(),
        Some(TooDeep {
            max_depth: 2,
            pos: 1
        })
    );
    assert_eq!(error_outline(cursor.error()), "c@1..1");
    assert!(cursor.ancestors().is_empty());
}

#[test]
fn the_too_deep_message_says_how_to_fix_it() {
    let message = TooDeep {
        max_depth: 4,
        pos: 1,
    }
    .to_string();

    assert!(message.contains("max_depth"), "{message}");
    assert!(message.contains("stack"), "{message}");
}

#[test]
fn a_sequence_whose_end_probe_halted_returns_no_node() {
    // Driven directly there is no `exec` to drop the tree: the sequence
    // itself must fail once its lookahead's probe halted.
    let mut pattern = seq(
        "s",
        vec![lit("a", "a"), not("n", choice("c", vec![lit("b", "b")]))],
    );
    let mut cursor = Cursor::with_limits("a", limited(2));

    assert!(pattern.parse(&mut cursor).is_none());
    assert!(cursor.halted());
}

/// The verifier's grammar: one operator of each kind over `a`.
/// `e = "-" e | e "!" | e "+" e | RightAssociated(e "^" e) | "a"`.
fn operators() -> Pattern {
    choice(
        "e",
        vec![
            seq("neg", vec![lit("-", "-"), reference("e")]),
            seq("bang", vec![reference("e"), lit("!", "!")]),
            seq("add", vec![reference("e"), lit("+", "+"), reference("e")]),
            right(seq(
                "pow",
                vec![reference("e"), lit("^", "^"), reference("e")],
            )),
            lit("a", "a"),
        ],
    )
}

/// Nodes on the longest root-to-leaf path, counted without recursing.
fn depth(root: Node<'_, '_>) -> usize {
    let mut deepest = 0;
    let mut pending = vec![(root, 1)];
    while let Some((node, at)) = pending.pop() {
        deepest = deepest.max(at);
        pending.extend(node.children().map(|child| (child, at + 1)));
    }
    deepest
}

/// The Pratt loop is iterative, so the depth limit never sees an operator
/// chain: `a+a+…+a` parses into a tree as deep as the input is long. Writing
/// that tree recursed once per level and aborted the process with a stack
/// overflow (a debug build on a test thread's stack overflowed well before
/// 100 000 operators). The tree is written, walked, flattened and printed
/// without recursion, so every chain parses in full.
#[test]
fn an_operator_chain_as_deep_as_the_input_parses_without_overflowing() {
    const N: usize = 100_000;
    let chains = [
        (
            "left-associated infix",
            format!("a{}", "+a".repeat(N)),
            "add",
        ),
        (
            "right-associated infix",
            format!("a{}", "^a".repeat(N)),
            "pow",
        ),
        ("prefix", format!("{}a", "-".repeat(N)), "neg"),
        ("postfix", format!("a{}", "!".repeat(N)), "bang"),
    ];

    for (kind, text, operator) in chains {
        let mut pattern = operators();

        let result = pattern.exec(&text);

        let tree = result.tree().unwrap_or_else(|| panic!("{kind}: no tree"));
        let root = tree.root();
        assert_eq!(root.name(), operator, "{kind}");
        assert_eq!(root.value(), text, "{kind}");
        assert_eq!(depth(root), N + 1, "{kind}: one level per operator");
        assert_eq!(
            root.find_all(|n| n.kind() == NodeKind::Expression).len(),
            N,
            "{kind}"
        );
        assert_eq!(
            root.find(|n| n.is_leaf()).map(|n| n.name()),
            Some(if kind == "prefix" { "-" } else { "a" }),
            "{kind}"
        );
        assert!(format!("{root:?}").contains("children: "), "{kind}");
    }
}

#[test]
fn printing_a_node_shows_it_and_its_child_count_not_the_subtree() {
    let mut pattern = operators();
    let result = pattern.exec("a+a");
    let root = result.tree().unwrap().root();

    assert_eq!(
        format!("{root:?}"),
        r#"Node { kind: Expression, name: "add", range: 0..3, value: "a+a", children: 3 }"#
    );
}
