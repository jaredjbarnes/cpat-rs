//! v12's `Expression.test.ts` parse behavior, on the one `Choice` (its
//! Pratt and ordered modes), with hand-built references. Expected trees and
//! errors come from running v12 on the same grammars and inputs.
//!
//! Deferred: the introspection tests (`getTokens`, `getTokensAfter`,
//! `getPatterns`, `getPatternsAfter`, `getNextTokens`, `getNextPatterns`,
//! `find`, `isEqual`, "Options like", "Suggest", "continuation tokens") to
//! the intellisense milestone (M8), and the "literal-set fast path" tests to
//! M5 (`LiteralSet` mode).

mod common;

use common::*;
use cpat_core::{Choice, ChoiceMode, Cursor, Limits, NodeKind, Pattern, PatternError};

fn e() -> Pattern {
    reference("expression")
}

/// v12's `createExpressionPattern`.
fn expression_pattern() -> Pattern {
    let spaces = re("spaces", r"\s+");
    let optional_spaces = || opt("optional-spaces", spaces.clone());
    let variables = choice(
        "variables",
        ["a", "b", "c", "d", "e"].map(|v| lit(v, v)).to_vec(),
    );
    let mul_div = choice("mult-div", vec![lit("mult", " * "), lit("div", " / ")]);
    let add_sub = choice("add-sub", vec![lit("add", " + "), lit("sub", " - ")]);
    let and_or = choice("and-or", vec![lit("and", " && "), lit("or", " || ")]);
    let group = seq(
        "group",
        vec![
            lit("open-paren", "("),
            optional_spaces(),
            e(),
            optional_spaces(),
            lit("close-paren", ")"),
        ],
    );
    let ternary = seq(
        "ternary",
        vec![
            e(),
            optional_spaces(),
            lit("question-mark", "?"),
            optional_spaces(),
            e(),
            optional_spaces(),
            lit("colon", ":"),
            optional_spaces(),
            e(),
        ],
    );

    choice(
        "expression",
        vec![
            seq("mult-div-expression", vec![e(), mul_div, e()]),
            seq("add-sub-expression", vec![e(), add_sub, e()]),
            seq("bool-expression", vec![e(), and_or, e()]),
            right(ternary),
            group,
            variables,
        ],
    )
}

/// v12's `createTailExpression`.
fn tail_expression() -> Pattern {
    let variable = choice(
        "variable",
        vec![lit("a", "a"), lit("b", "b"), lit("c", "c")],
    );
    let refinement = seq("refinement", vec![lit(".", "."), variable.clone()]);
    choice(
        "expression",
        vec![
            seq("refinement-expression", vec![e(), refinement]),
            seq("invocation-expression", vec![e(), lit("invocation", "()")]),
            variable,
        ],
    )
}

fn negate_or_variable() -> Pattern {
    choice(
        "expression",
        vec![
            seq("negate", vec![lit("minus", "-"), e()]),
            lit("variable", "a"),
        ],
    )
}

fn factorial_or_variable() -> Pattern {
    choice(
        "expression",
        vec![
            seq("factorial", vec![e(), lit("bang", "!")]),
            lit("variable", "a"),
        ],
    )
}

fn names(alternatives: Vec<Pattern>) -> Pattern {
    choice("names", alternatives)
}

fn full_name() -> Pattern {
    let first = choice("first-name", vec![lit("john", "John"), lit("jane", "Jane")]);
    let last = choice("last-name", vec![lit("doe", "Doe"), lit("smith", "Smith")]);
    seq("full-name", vec![first, lit("space", " "), last])
}

fn the_choice(pattern: &Pattern) -> &Choice {
    as_choice(pattern)
}

// ─── Expression Pattern ─────────────────────────────────────────────────

#[test]
fn single_expression() {
    let mut expression = expression_pattern();
    let valid = [
        "a || c || b / c * a + d",
        "a + b",
        "a + b * c * d",
        "a + b * c || d + e",
        "(a + b) * (c + d)",
        "(a + b) * c + (d + e)",
        "a + b * c ? d : e",
        "a + b * (a + b * c ? d : e) ? d : e",
        "a + b * a + b * c ? d : e ? d : e",
    ];

    for input in valid {
        let result = expression.exec(input);
        assert!(result.error.is_none(), "{input}");
        assert_eq!(result.tree().unwrap().root().value(), input);
    }

    // "*" is declared before "+", so it binds tighter and nests deeper.
    assert_eq!(
        exec_shape(&mut expression, "a + b * c"),
        "add-sub-expression:expression@0..9[a:literal@0..1 add:literal@1..4 \
         mult-div-expression:expression@4..9[b:literal@4..5 mult:literal@5..8 c:literal@8..9]]"
    );
    assert_eq!(
        exec_shape(&mut expression, "a || c || b / c * a + d"),
        "bool-expression:expression@0..23[bool-expression:expression@0..6[a:literal@0..1 \
         or:literal@1..5 c:literal@5..6] or:literal@6..10 add-sub-expression:expression@10..23[\
         mult-div-expression:expression@10..19[mult-div-expression:expression@10..15[\
         b:literal@10..11 div:literal@11..14 c:literal@14..15] mult:literal@15..18 \
         a:literal@18..19] add:literal@19..22 d:literal@22..23]]"
    );

    assert_eq!(
        exec_outline(&mut expression, "a + b * ?"),
        outlined("-", "expression@5..9", "open-paren@8..8")
    );
}

#[test]
fn a_ternary_takes_whole_expressions_as_operands() {
    let mut expression = expression_pattern();

    assert_eq!(
        exec_shape(&mut expression, "a + b * c ? d : e"),
        "ternary:expression@0..17[add-sub-expression:expression@0..9[a:literal@0..1 \
         add:literal@1..4 mult-div-expression:expression@4..9[b:literal@4..5 mult:literal@5..8 \
         c:literal@8..9]] spaces:regex@9..10 question-mark:literal@10..11 spaces:regex@11..12 \
         d:literal@12..13 spaces:regex@13..14 colon:literal@14..15 spaces:regex@15..16 \
         e:literal@16..17]"
    );
    assert_eq!(
        exec_outline(&mut expression, "(a"),
        outlined("-", "expression@0..2", "group@0..2")
    );
    assert_eq!(
        exec_outline(&mut expression, "a +"),
        outlined("-", "expression@1..3", "mult@1..2")
    );
}

#[test]
fn tail() {
    let mut expression = tail_expression();

    for input in ["a", "a.b", "a.b.c", "a.b.c()()()"] {
        let result = expression.exec(input);
        assert!(result.error.is_none(), "{input}");
        assert_eq!(result.tree().unwrap().root().value(), input);
    }

    // Postfix chains are left associative, so "a.b.c" nests as "(a.b).c".
    assert_eq!(
        exec_shape(&mut expression, "a.b.c"),
        "refinement-expression:expression@0..5[refinement-expression:expression@0..3[\
         a:literal@0..1 refinement:sequence@1..3[.:literal@1..2 b:literal@2..3]] \
         refinement:sequence@3..5[.:literal@3..4 c:literal@4..5]]"
    );
    assert_eq!(
        exec_outline(&mut expression, "a."),
        outlined("-", "expression@1..2", "refinement@1..2")
    );
}

#[test]
fn clone_with_new_name() {
    let original = expression_pattern();
    let mut clone = original.renamed("new-expression");

    assert_eq!(clone.name(), "new-expression");
    assert!(matches!(clone, Pattern::Choice(_)));

    // Deviation (DESIGN §11, unknown names up front): the alternatives
    // still refer to `expression`, which nothing in the renamed tree is
    // called. v12 finds out only when a parse reaches one (`(a)+a` throws);
    // its test passes because its input never does. Here `exec` says so for
    // every input.
    assert!(matches!(
        clone.exec("a || b && c * d").pattern_error,
        Some(PatternError::UnknownReference { .. })
    ));

    // With `expression` defined, the clone keeps the original's precedence:
    // "*" binds tightest, while "||" and "&&" share a level and so nest to
    // the left.
    let mut scoped = context("scope", clone, vec![original]);
    assert_eq!(
        exec_shape(&mut scoped, "a || b && c * d"),
        "bool-expression:expression@0..15[bool-expression:expression@0..6[a:literal@0..1 \
         or:literal@1..5 b:literal@5..6] and:literal@6..10 mult-div-expression:expression@10..15[\
         c:literal@10..11 mult:literal@11..14 d:literal@14..15]]"
    );
}

#[test]
fn empty_patterns_are_rejected() {
    assert!(matches!(
        Choice::new("e", vec![]),
        Err(PatternError::NoChildren { .. })
    ));
}

#[test]
fn an_atom_only_expression_parses_a_single_value_and_stops() {
    let mut expression = choice(
        "expression",
        vec![lit("a", "a"), lit("b", "b"), lit("c", "c")],
    );

    assert_eq!(
        exec_outline(&mut expression, "a"),
        outlined("a@0..1", "-", "-")
    );
}

#[test]
fn prefix_at_end_of_input() {
    let mut expression = negate_or_variable();

    assert_eq!(
        exec_shape(&mut expression, "-a"),
        "negate:expression@0..2[minus:literal@0..1 variable:literal@1..2]"
    );
    assert_eq!(
        exec_shape(&mut expression, "--a"),
        "negate:expression@0..3[minus:literal@0..1 negate:expression@1..3[minus:literal@1..2 \
         variable:literal@2..3]]"
    );
}

#[test]
fn infix_at_end_of_input() {
    let result = expression_pattern().exec("a + b");

    assert!(result.error.is_none());
    assert!(result.tree().is_some());
}

#[test]
fn postfix_operator() {
    assert_eq!(
        exec_shape(&mut factorial_or_variable(), "a!"),
        "factorial:expression@0..2[variable:literal@0..1 bang:literal@1..2]"
    );
}

#[test]
fn chained_postfix_operators() {
    let mut expression = factorial_or_variable();

    assert_eq!(
        exec_shape(&mut expression, "a!!"),
        "factorial:expression@0..3[factorial:expression@0..2[variable:literal@0..1 \
         bang:literal@1..2] bang:literal@2..3]"
    );
    assert_eq!(
        exec_outline(&mut expression, "a!b"),
        outlined("-", "expression@2..3", "bang@2..2")
    );
}

#[test]
fn an_expression_with_no_atoms_matches_nothing() {
    // Every alternative is infix, so nothing can start an expression.
    let mut expression = choice(
        "expression",
        vec![seq("add", vec![e(), lit("plus", "+"), e()])],
    );

    assert_eq!(the_choice(&expression).atom_patterns().count(), 0);
    assert_eq!(
        exec_outline(&mut expression, "a + b"),
        outlined("-", "expression@0..5", "expression@0..0")
    );
}

#[test]
fn a_choice_with_no_operands_tries_nothing() {
    // Only a prefix: nothing can complete an expression, so the operator is
    // never tried and the choice's own error is the only one.
    let mut expression = choice("expression", vec![seq("neg", vec![lit("-", "-"), e()])]);

    assert_eq!(
        exec_outline(&mut expression, "x"),
        outlined("-", "expression@0..1", "expression@0..0")
    );
    assert_eq!(
        exec_outline(&mut expression, "-x"),
        outlined("-", "expression@0..2", "expression@0..0")
    );
}

#[test]
fn a_halt_inside_an_operator_fails_the_choice() {
    // The operand `a` is already in the tree when the operator's frame goes
    // past the depth limit; the halt must not leave that operand as a match.
    let mut expr = choice(
        "expr",
        vec![
            seq(
                "add",
                vec![reference("expr"), lit("+", "+"), reference("expr")],
            ),
            lit("a", "a"),
        ],
    );
    let mut cursor = Cursor::with_limits("a+a", Limits { max_depth: 1 });

    assert!(expr.parse(&mut cursor).is_none());
    assert!(cursor.too_deep().is_some());
}

#[test]
fn test_returns_a_boolean() {
    let mut expression = expression_pattern();

    assert!(expression.test("a + b"));
    assert!(expression.test("a + b * c"));
    assert!(!expression.test("???"));
}

#[test]
fn accessors_sort_the_alternatives_by_role() {
    let expression = choice(
        "expression",
        vec![
            seq("add", vec![e(), lit("plus", "+"), e()]),
            seq("negate", vec![lit("minus", "-"), e()]),
            seq("factorial", vec![e(), lit("bang", "!")]),
            lit("variable", "a"),
        ],
    );
    let choice = the_choice(&expression);
    let names = |patterns: Vec<&str>| patterns.join(",");

    assert_eq!(choice.mode(), Some(ChoiceMode::Pratt));
    assert_eq!(
        names(choice.prefix_patterns().map(|p| p.name()).collect()),
        "negate-prefix"
    );
    assert_eq!(
        names(choice.postfix_patterns().map(|p| p.name()).collect()),
        "factorial-postfix"
    );
    assert_eq!(
        names(choice.infix_patterns().map(|p| p.name()).collect()),
        "add-delimiter"
    );
    assert_eq!(
        names(choice.atom_patterns().map(Pattern::name).collect()),
        "variable"
    );
    // The operator parts are what is left without the self-references.
    let delimiter = choice.infix_patterns().next().unwrap();
    assert_eq!(delimiter.children().len(), 1);
    assert_eq!(delimiter.children()[0].name(), "plus");
}

#[test]
fn original_alternatives_are_the_alternatives_as_given() {
    let expression = choice("expression", vec![lit("a", "a"), lit("b", "b")]);
    let names: Vec<&str> = the_choice(&expression)
        .original_alternatives()
        .iter()
        .map(Pattern::name)
        .collect();

    assert_eq!(names, ["a", "b"]);
}

#[test]
fn a_prefix_matched_but_the_input_ends_before_an_atom() {
    let mut expression = negate_or_variable();

    assert_eq!(
        exec_outline(&mut expression, "-"),
        outlined("-", "expression@0..1", "expression@0..0")
    );
}

#[test]
fn an_atom_only_expression_with_trailing_input_stops_at_the_binary_check() {
    let mut expression = choice("expression", vec![lit("a", "a")]);

    assert_eq!(
        exec_outline(&mut expression, "ab"),
        outlined("-", "expression@1..2", "expression@1..2")
    );
}

#[test]
fn a_right_associated_binary_nests_right_with_a_tighter_operator_between() {
    // `add` is tighter than a right-associated `as`: in `a as b + c as d`
    // the `b + c` binds first, and the two `as` nest to the right.
    let variables = choice(
        "variables",
        ["a", "b", "c", "d"].map(|v| lit(v, v)).to_vec(),
    );
    let mut expression = choice(
        "expression",
        vec![
            seq("add-expression", vec![e(), lit("add", " + "), e()]),
            right(seq("as-expression", vec![e(), lit("as", " as "), e()])),
            variables,
        ],
    );

    assert_eq!(
        exec_shape(&mut expression, "a as b + c as d"),
        "as-expression:expression@0..15[a:literal@0..1 as:literal@1..5 \
         as-expression:expression@5..15[add-expression:expression@5..10[b:literal@5..6 \
         add:literal@6..9 c:literal@9..10] as:literal@10..14 d:literal@14..15]]"
    );
}

// ─── Expression: ordered choice (atoms only) ────────────────────────────

#[test]
fn one_alternative_successful_returns_the_alternatives_node_unwrapped() {
    let mut a = choice("a", vec![lit("a", "A")]);

    assert_eq!(the_choice(&a).mode(), Some(ChoiceMode::Ordered));
    assert_eq!(parse_shape(&mut a, "A").0, "a:literal@0..1");
}

#[test]
fn one_alternative_failed_records_an_error_and_rewinds() {
    let mut a = choice("a", vec![lit("a", "A")]);
    let mut cursor = Cursor::new("B");

    assert!(a.parse(&mut cursor).is_none());
    assert_eq!(cursor.pos(), 0);
    assert_eq!(error_outline(cursor.error()), "a@0..0");
}

#[test]
fn two_alternatives_each_can_match() {
    let mut a_or_b = choice("a-or-b", vec![lit("a", "A"), lit("b", "B")]);
    let mut cursor = Cursor::new("AB");

    let first = a_or_b.parse(&mut cursor).unwrap();
    let second = a_or_b.parse(&mut cursor).unwrap();

    let tree = cursor.into_tree(second);
    assert_eq!(shape(tree.node(first)), "a:literal@0..1");
    assert_eq!(shape(tree.root()), "b:literal@1..2");
}

#[test]
fn the_first_matching_alternative_wins_even_when_a_later_one_is_longer() {
    let mut op = choice("op", vec![lit("eq", "="), lit("eqeq", "==")]);

    assert_eq!(parse_shape(&mut op, "==").0, "eq:literal@0..1");
}

#[test]
fn a_failing_earlier_alternative_falls_through_to_a_later_one() {
    let johnson = || lit("john-johnson", "John Johnson");
    let stockton = || lit("john-stockton", "John Stockton");

    assert_eq!(
        exec_shape(&mut names(vec![full_name(), johnson()]), "John Johnson"),
        "john-johnson:literal@0..12"
    );
    assert_eq!(
        exec_shape(&mut names(vec![johnson(), full_name()]), "John Johnson"),
        "john-johnson:literal@0..12"
    );
    let mut middle = names(vec![stockton(), johnson(), full_name()]);
    assert_eq!(
        exec_shape(&mut middle, "John Johnson"),
        "john-johnson:literal@0..12"
    );
    assert_eq!(
        exec_outline(&mut middle, "Jane Doe"),
        outlined(
            "full-name@0..8[jane@0..4 space@4..5 doe@5..8]",
            "-",
            "john-stockton@0..1"
        )
    );
}

#[test]
fn an_optional_alternative_that_matches_nothing_does_not_block_a_later_one() {
    let mut first_name = choice(
        "first-name",
        vec![
            opt("optional-john", lit("john", "John")),
            lit("jane", "Jane"),
        ],
    );

    assert_eq!(
        exec_outline(&mut first_name, "Jane"),
        outlined("jane@0..4", "-", "john@0..1")
    );
}

#[test]
fn recursion_through_a_sequence() {
    let group = seq("group", vec![lit("open", "("), e(), lit("close", ")")]);
    let mut expression = choice("expression", vec![group, lit("john", "John")]);

    assert_eq!(the_choice(&expression).mode(), Some(ChoiceMode::Ordered));
    assert!(expression.test("John"));
    assert_eq!(
        exec_outline(&mut expression, "(((John)))"),
        outlined(
            "group@0..10[open@0..1 group@1..9[open@1..2 group@2..8[open@2..3 john@3..7 \
             close@7..8] close@8..9] close@9..10]",
            "-",
            "open@3..3"
        )
    );
    assert_eq!(
        exec_outline(&mut expression, "((John)"),
        outlined("-", "expression@0..7", "group@0..7")
    );
}

#[test]
fn properties() {
    let a = choice("a", vec![lit("a", "A")]);

    assert_eq!(a.name(), "a");
    assert_eq!(the_choice(&a).atom_patterns().next().unwrap().name(), "a");
}

#[test]
fn exec_and_test() {
    let mut a = choice("a", vec![lit("a", "A")]);

    assert!(a.exec("B").tree.is_none());
    assert!(!a.test("B"));
    assert!(a.test("A"));
}

/// `a = b | "c"`, `b = a | "d"`.
fn mutually_recursive() -> Pattern {
    let b = choice("b", vec![reference("a"), lit("d", "d")]);
    choice("a", vec![b, lit("c", "c")])
}

#[test]
fn mutually_recursive_choices_parse() {
    // Each choice leads back into the other at the same index. The
    // recursion guard skips the alternative already being tried there, so
    // the other alternative gets its turn; no stack overflow.
    let mut a = mutually_recursive();

    assert_eq!(exec_outline(&mut a, "c"), outlined("c@0..1", "-", "-"));
    assert_eq!(exec_outline(&mut a, "d"), outlined("d@0..1", "-", "c@0..0"));
    assert_eq!(exec_outline(&mut a, "x"), outlined("-", "a@0..1", "c@0..0"));
}

#[test]
fn a_self_referencing_choice_alternative_is_an_atom_not_an_operator() {
    // `b = a | "d"` merely contains `a`; it is not the postfix `a "d"`.
    // Only sequences classify as operators.
    let mut a = mutually_recursive();

    let classified = the_choice(&a);
    assert_eq!(classified.mode(), Some(ChoiceMode::Ordered));
    let atoms: Vec<&str> = classified.atom_patterns().map(Pattern::name).collect();
    assert_eq!(atoms, ["b", "c"]);
    assert_eq!(classified.postfix_patterns().count(), 0);

    assert_eq!(exec_outline(&mut a, "d"), outlined("d@0..1", "-", "c@0..0"));
    // `b` has a reference alternative, so it was classified at its first
    // parse: its target `a` is a choice, so an atom too.
    let b = the_choice(&the_choice(&a).original_alternatives()[0]);
    assert_eq!(b.mode(), Some(ChoiceMode::Ordered));
}

#[test]
fn cyclical_error_recovery() {
    let space = re("space", r"\s+");
    let r = e();
    let names = choice("names", vec![lit("john", "John"), lit("jane", "Jane")]);
    let ternary = seq(
        "ternary",
        vec![
            r.clone(),
            space.clone(),
            lit("?", "?"),
            space.clone(),
            r.clone(),
            space.clone(),
            lit(":", ":"),
            space,
            r,
        ],
    );
    let mut expression = choice("expression", vec![names, ternary]);

    assert_eq!(exec_shape(&mut expression, "John"), "john:literal@0..4");
    assert_eq!(
        exec_shape(&mut expression, "John ? Jane : John"),
        "ternary:expression@0..18[john:literal@0..4 space:regex@4..5 ?:literal@5..6 \
         space:regex@6..7 jane:literal@7..11 space:regex@11..12 ::literal@12..13 \
         space:regex@13..14 john:literal@14..18]"
    );
    let nested = "John ? John ? Jane : John ? Jane : John : John";
    assert_eq!(
        exec_shape(&mut expression, nested),
        "ternary:expression@0..46[john:literal@0..4 space:regex@4..5 ?:literal@5..6 \
         space:regex@6..7 ternary:expression@7..39[ternary:expression@7..25[john:literal@7..11 \
         space:regex@11..12 ?:literal@12..13 space:regex@13..14 jane:literal@14..18 \
         space:regex@18..19 ::literal@19..20 space:regex@20..21 john:literal@21..25] \
         space:regex@25..26 ?:literal@26..27 space:regex@27..28 jane:literal@28..32 \
         space:regex@32..33 ::literal@33..34 space:regex@34..35 john:literal@35..39] \
         space:regex@39..40 ::literal@40..41 space:regex@41..42 john:literal@42..46]"
    );
    // The middle operand is an expression too, so its failure is the
    // delimiter's.
    assert_eq!(
        exec_outline(&mut expression, "John ? Jane"),
        outlined("-", "expression@4..11", "ternary-delimiter@4..11")
    );
}

// ─── Lessons from v12 ───────────────────────────────────────────────────

#[test]
fn known_limitation_a_prefix_consuming_the_last_token_does_not_backtrack() {
    // `x = "a" x | "a"`: the prefix takes `a`, and no operand is left. v12
    // rejects `a` the same way; pinned so a change is deliberate.
    let mut x = choice(
        "x",
        vec![
            seq("ax", vec![lit("a", "a"), reference("x")]),
            lit("a", "a"),
        ],
    );

    assert_eq!(exec_outline(&mut x, "a"), outlined("-", "x@0..1", "x@0..0"));
    assert_eq!(
        exec_outline(&mut x, "aa"),
        outlined("-", "x@0..2", "x@0..0")
    );
}

#[test]
fn a_sequence_starting_with_the_choice_is_a_postfix_operator() {
    // `a = a "b" | "b"`: v12 makes `ab` a postfix, so left recursion parses
    // as a left-nested chain.
    let mut a = choice(
        "a",
        vec![
            seq("ab", vec![reference("a"), lit("b", "b")]),
            lit("b", "b"),
        ],
    );

    assert_eq!(
        exec_shape(&mut a, "bbb"),
        "ab:expression@0..3[ab:expression@0..2[b:literal@0..1 b:literal@1..2] b:literal@2..3]"
    );
    assert_eq!(
        exec_outline(&mut a, "bc"),
        outlined("-", "a@1..2", "b@1..1")
    );
}

// ─── Classification ─────────────────────────────────────────────────────

/// `expr = add | num`, with both alternatives references into a context.
fn referenced_alternatives() -> Pattern {
    context(
        "root",
        choice("expr", vec![reference("add"), reference("num")]),
        vec![
            seq(
                "add",
                vec![reference("expr"), lit("+", "+"), reference("expr")],
            ),
            re("num", "[0-9]"),
        ],
    )
}

fn inner_choice(pattern: &Pattern) -> &Choice {
    match pattern {
        Pattern::Context(context) => as_choice(context.child()),
        other => as_choice(other),
    }
}

#[test]
fn a_reference_alternative_is_classified_by_its_target_at_the_first_parse() {
    // Whether `add` is an operator depends on what the name resolves to,
    // so the choice waits for the scopes its first parse has.
    let mut root = referenced_alternatives();
    assert_eq!(inner_choice(&root).mode(), None);
    assert_eq!(inner_choice(&root).atom_patterns().count(), 0);

    assert_eq!(
        exec_shape(&mut root, "1+2+3"),
        "add:expression@0..5[add:expression@0..3[num:regex@0..1 +:literal@1..2 num:regex@2..3] \
         +:literal@3..4 num:regex@4..5]"
    );
    let expr = inner_choice(&root);
    assert_eq!(expr.mode(), Some(ChoiceMode::Pratt));
    assert_eq!(
        expr.infix_patterns().next().unwrap().name(),
        "add-delimiter"
    );
    assert_eq!(
        exec_outline(&mut root, "1+"),
        outlined("-", "expr@1..2", "expr@1..2")
    );
}

#[test]
fn a_reference_alternative_names_its_operator_nodes() {
    // `plus` refers to `add`: the operator is the reference's, as v12's
    // resolved clone takes the reference's name.
    let mut root = context(
        "root",
        choice(
            "expr",
            vec![reference_to("plus", "add"), re("num", "[0-9]")],
        ),
        vec![seq(
            "add",
            vec![reference("expr"), lit("+", "+"), reference("expr")],
        )],
    );

    assert_eq!(
        exec_shape(&mut root, "1+2+3"),
        "plus:expression@0..5[plus:expression@0..3[num:regex@0..1 +:literal@1..2 \
         num:regex@2..3] +:literal@3..4 num:regex@4..5]"
    );
}

#[test]
fn a_reference_to_a_choice_is_an_atom() {
    let mut root = context(
        "root",
        choice(
            "expr",
            vec![
                reference("num"),
                seq(
                    "add",
                    vec![reference("expr"), lit("+", "+"), reference("expr")],
                ),
            ],
        ),
        vec![choice(
            "num",
            vec![
                re("d", "[0-9]"),
                seq("neg", vec![lit("-", "-"), reference("num")]),
            ],
        )],
    );

    // `num` is a Pratt choice of its own, so `neg` nests inside the operand.
    assert_eq!(
        exec_shape(&mut root, "1+-2"),
        "add:expression@0..4[d:regex@0..1 +:literal@1..2 neg:expression@2..4[-:literal@2..3 \
         d:regex@3..4]]"
    );
}

#[test]
fn a_renamed_choice_still_recognises_its_self_references() {
    // Classification reads the name the choice was built with (v12
    // `_originalName`), so a renamed clone waiting to classify still finds
    // `expr` in `add`.
    let Pattern::Context(context) = referenced_alternatives() else {
        unreachable!()
    };
    let renamed = context.child().renamed("other");
    let definitions = context
        .patterns_within_context()
        .into_values()
        .cloned()
        .chain([renamed.renamed("expr")])
        .collect();
    let mut root = common::context("root", renamed, definitions);

    assert_eq!(
        exec_shape(&mut root, "1+2"),
        "add:expression@0..3[num:regex@0..1 +:literal@1..2 num:regex@2..3]"
    );
    assert_eq!(inner_choice(&root).mode(), Some(ChoiceMode::Pratt));
}

#[test]
fn a_sequence_of_only_the_choice_itself_is_rejected() {
    let only_self = Choice::new(
        "expr",
        vec![seq("only", vec![reference("expr")]), lit("a", "a")],
    );

    assert_eq!(
        only_self.unwrap_err(),
        PatternError::SelfOnlyAlternative {
            choice: "expr".to_string(),
            alternative: "only".to_string(),
        }
    );
}

#[test]
fn a_reference_to_a_sequence_of_only_the_choice_halts_the_parse() {
    // Found only once the reference resolves, so it comes back from the
    // parse, as data.
    let mut root = context(
        "root",
        choice("expr", vec![reference("only"), lit("a", "a")]),
        vec![seq("only", vec![reference("expr")])],
    );

    let result = root.exec("a");
    assert!(result.tree.is_none());
    assert!(matches!(
        result.pattern_error,
        Some(PatternError::SelfOnlyAlternative { .. })
    ));
}

#[test]
fn an_alternative_that_is_neither_operand_nor_operator_is_never_tried() {
    // `expr expr` starts and ends with the choice but has no operator part:
    // v12 drops it.
    let mut expr = choice(
        "expr",
        vec![
            seq("pair", vec![reference("expr"), reference("expr")]),
            seq(
                "add",
                vec![reference("expr"), lit("+", "+"), reference("expr")],
            ),
            lit("a", "a"),
        ],
    );

    let names: Vec<&str> = the_choice(&expr)
        .atom_patterns()
        .map(Pattern::name)
        .collect();
    assert_eq!(names, ["a"]);
    assert_eq!(
        exec_outline(&mut expr, "aa"),
        outlined("-", "expr@1..2", "+@1..1")
    );
    assert_eq!(
        exec_shape(&mut expr, "a+a"),
        "add:expression@0..3[a:literal@0..1 +:literal@1..2 a:literal@2..3]"
    );
}

#[test]
fn operator_nodes_are_expression_nodes_and_operands_keep_their_kind() {
    let mut expr = choice(
        "expr",
        vec![
            seq(
                "add",
                vec![reference("expr"), lit("+", "+"), reference("expr")],
            ),
            seq("grp", vec![lit("(", "("), reference("expr"), lit(")", ")")]),
            re("num", "[0-9]"),
        ],
    );
    let result = expr.exec("(1+2)+3");
    let root = result.tree().unwrap().root();

    assert_eq!(root.kind(), NodeKind::Expression);
    assert_eq!(root.value(), "(1+2)+3");
    let kinds: Vec<NodeKind> = root.children().map(|c| c.kind()).collect();
    assert_eq!(
        kinds,
        [NodeKind::Sequence, NodeKind::Literal, NodeKind::Regex]
    );
    // The operator's sequence node is not in the tree: its children are.
    assert_eq!(
        root.children().nth(1).unwrap().parent().unwrap().name(),
        "add"
    );
}

#[test]
fn a_pratt_choice_inside_a_sequence_ends_where_its_tree_ends() {
    let expr = choice(
        "expr",
        vec![
            seq(
                "add",
                vec![reference("expr"), lit("+", "+"), reference("expr")],
            ),
            re("num", "[0-9]"),
        ],
    );
    let mut statement = seq("stmt", vec![expr, lit(";", ";")]);

    assert_eq!(
        exec_shape(&mut statement, "1+2;"),
        "stmt:sequence@0..4[add:expression@0..3[num:regex@0..1 +:literal@1..2 num:regex@2..3] \
         ;:literal@3..4]"
    );
    // A dangling `+` is dropped, so the sequence goes on from after `1`.
    assert_eq!(
        exec_outline(&mut statement, "1+;"),
        outlined("-", "stmt@0..3", "num@2..2")
    );
}

#[test]
fn the_atom_guard_skips_an_atom_already_parsing_here() {
    // `b = expr | "x"` is an atom of `expr`, and leads straight back into
    // it. In the inner `expr`, the guard skips `b` (already parsing at this
    // spot), which decides the furthest error as in v12.
    let mut expr = choice(
        "expr",
        vec![
            seq(
                "add",
                vec![reference("expr"), lit("+", "+"), reference("expr")],
            ),
            choice("b", vec![reference("expr"), lit("x", "x")]),
            re("num", "[0-9]"),
        ],
    );

    assert_eq!(
        exec_shape(&mut expr, "1+x"),
        "add:expression@0..3[num:regex@0..1 +:literal@1..2 x:literal@2..3]"
    );
    assert_eq!(
        exec_outline(&mut expr, "y"),
        outlined("-", "expr@0..1", "num@0..0")
    );
}
