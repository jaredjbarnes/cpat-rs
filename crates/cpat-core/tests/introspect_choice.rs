//! Introspection of choices, ported from v12 `Expression.test.ts` and
//! `RightAssociated.test.ts` (each test names its source). A choice's
//! children are v12's classified children (`Pattern::at_path`): operands
//! are the alternatives, operators their sequences without the
//! self-references (`<name>-prefix`, `-postfix`, `-delimiter`), in
//! declaration order.
//!
//! Where v12's test only checks `toContain` or a non-empty result, the
//! exact v12 answer (printed by running v12 on the same pattern) is pinned.
//! 'Suggest' needs `AutoComplete`, a later milestone.

mod common;

use common::*;
use cpat_core::{Pattern, PatternPath};

fn negate() -> Pattern {
    seq("negate", vec![lit("minus", "-"), reference("expression")])
}

fn factorial() -> Pattern {
    seq("factorial", vec![reference("expression"), lit("bang", "!")])
}

fn add() -> Pattern {
    seq(
        "add",
        vec![
            reference("expression"),
            lit("plus", "+"),
            reference("expression"),
        ],
    )
}

fn variable() -> Pattern {
    lit("variable", "x")
}

fn expression(alternatives: Vec<Pattern>) -> Pattern {
    choice("expression", alternatives)
}

/// v12 `createOptionsExpression`.
fn options() -> Pattern {
    expression(vec![lit("a", "a"), lit("b", "b"), lit("c", "c")])
}

/// v12's `wrapper = expression ";"`.
fn wrapped(expression: Pattern) -> Pattern {
    seq("wrapper", vec![expression, lit("end", ";")])
}

// ─── Expression.test.ts ───

#[test]
fn get_tokens_returns_atom_and_prefix_tokens() {
    // Expression.test.ts 'getTokens returns atom and prefix tokens'
    let root = expression(vec![
        negate(),
        choice("variable", vec![lit("a", "a"), lit("b", "b")]),
    ]);

    assert_eq!(root.tokens(), ["-", "a", "b"]);
    assert_eq!(names(&root.patterns()), ["minus", "a", "b"]);
}

#[test]
fn the_classified_children_are_numbered_in_declaration_order() {
    // Expression.test.ts 'Accessor: infixPatterns, prefixPatterns,
    // postfixPatterns' (the accessors are pinned in expression.rs); here,
    // the children a path walks. `negate` is a prefix, `factorial` a
    // postfix, `add` an infix.
    let root = expression(vec![add(), negate(), factorial(), variable()]);
    let intro = root.introspect();
    let child = |i: u32| intro.at(&at(&[i])).map(|h| h.name());

    assert_eq!(child(0), Some("add-delimiter"));
    assert_eq!(child(1), Some("negate-prefix"));
    assert_eq!(child(2), Some("factorial-postfix"));
    assert_eq!(child(3), Some("variable"));
    assert_eq!(child(4), None);
    // An operator's children are its part of the sequence.
    assert_eq!(intro.at(&at(&[0, 0])).map(|h| h.name()), Some("plus"));
    assert_eq!(intro.at(&at(&[0, 1])).map(|h| h.name()), None);
}

#[test]
fn get_tokens_after_for_prefix_child() {
    // Expression.test.ts 'getTokensAfter for prefix child returns atoms and
    // prefix tokens'. v12: `["-", "x"]`.
    let root = expression(vec![negate(), variable()]);

    assert_eq!(root.tokens_after(0), ["-", "x"]);
}

#[test]
fn get_tokens_after_for_atom_child() {
    // Expression.test.ts 'getTokensAfter for atom child returns postfix and
    // infix tokens'. Children: add-delimiter, factorial-postfix, variable.
    // v12: `["!", "+"]` (postfixes first).
    let root = expression(vec![add(), factorial(), variable()]);

    assert_eq!(root.tokens_after(2), ["!", "+"]);
}

#[test]
fn get_tokens_after_for_postfix_child() {
    // Expression.test.ts 'getTokensAfter for postfix child returns postfix
    // and infix tokens'. v12: `["!", "+"]`.
    let root = expression(vec![add(), factorial(), variable()]);

    assert_eq!(root.tokens_after(1), ["!", "+"]);
}

/// v12 `createPrefixAndInfixExpression`: negate-prefix, add-delimiter,
/// variable.
fn prefix_and_infix() -> Pattern {
    expression(vec![negate(), add(), variable()])
}

#[test]
fn get_tokens_after_for_an_infix_child_offers_prefixes_as_well_as_atoms() {
    // Expression.test.ts 'getTokensAfter for an infix child offers prefixes
    // as well as atoms'. v12: `["-", "x"]`.
    let root = prefix_and_infix();

    assert_eq!(root.tokens_after(1), ["-", "x"]);
}

#[test]
fn get_patterns_after_for_an_infix_child_matches_what_can_start_an_expression() {
    // Expression.test.ts 'getPatternsAfter for an infix child matches what
    // can start an expression'
    let root = prefix_and_infix();
    let after = root.patterns_after(1);
    let leading = root.patterns();

    assert_eq!(names(&after), ["minus", "variable"]);
    assert_eq!(paths(&after), paths(&leading));
}

#[test]
fn get_tokens_after_for_unknown_child_returns_empty() {
    // Expression.test.ts 'getTokensAfter for unknown child returns empty'
    assert!(options().tokens_after(3).is_empty());
}

#[test]
fn get_tokens_after_for_atom_child_with_parent_includes_parent_next_tokens() {
    // Expression.test.ts 'getTokensAfter for atom child with parent includes
    // parent next tokens'. v12: `[";"]`.
    let root = wrapped(expression(vec![variable()]));

    assert_eq!(root.introspect().tokens_after(&at(&[0]), 0), [";"]);
}

#[test]
fn get_patterns_returns_atom_and_prefix_patterns() {
    // Expression.test.ts 'getPatterns returns atom and prefix patterns'.
    // v12: `minus`, `variable`.
    let root = expression(vec![negate(), variable()]);
    let patterns = root.patterns();

    assert_eq!(names(&patterns), ["minus", "variable"]);
    assert_eq!(paths(&patterns), ["/0/0", "/1"]);
}

#[test]
fn get_patterns_after_for_prefix_child() {
    // Expression.test.ts 'getPatternsAfter for prefix child returns atom and
    // prefix patterns'. v12: `minus`, `variable`.
    let root = expression(vec![negate(), variable()]);

    assert_eq!(names(&root.patterns_after(0)), ["minus", "variable"]);
}

#[test]
fn get_patterns_after_for_atom_child() {
    // Expression.test.ts 'getPatternsAfter for atom child returns postfix
    // and infix patterns'. v12: `plus`.
    let root = expression(vec![add(), variable()]);

    assert_eq!(names(&root.patterns_after(1)), ["plus"]);
}

#[test]
fn get_patterns_after_for_atom_child_with_parent_includes_parent_next_patterns() {
    // Expression.test.ts 'getPatternsAfter for atom child with parent
    // includes parent next patterns'. v12: `end`.
    let root = wrapped(expression(vec![variable()]));
    let after = root.introspect().patterns_after(&at(&[0]), 0);

    assert_eq!(names(&after), ["end"]);
    assert_eq!(paths(&after), ["/1"]);
}

#[test]
fn get_patterns_after_for_postfix_child() {
    // Expression.test.ts 'getPatternsAfter for postfix child returns postfix
    // and infix patterns'. v12: `bang`, `plus`.
    let root = expression(vec![add(), factorial(), variable()]);

    assert_eq!(names(&root.patterns_after(1)), ["bang", "plus"]);
}

#[test]
fn get_patterns_after_for_postfix_child_with_parent_includes_parent_next_patterns() {
    // Expression.test.ts 'getPatternsAfter for postfix child with parent
    // includes parent next patterns'. v12: `bang`, `end`.
    let root = wrapped(expression(vec![factorial(), variable()]));

    let after = root.introspect().patterns_after(&at(&[0]), 0);
    assert_eq!(names(&after), ["bang", "end"]);
}

#[test]
fn get_tokens_after_for_postfix_child_with_parent_includes_parent_tokens() {
    // Expression.test.ts 'getTokensAfter for postfix child with parent
    // includes parent tokens'. v12: `["!", ";"]`.
    let root = wrapped(expression(vec![factorial(), variable()]));

    assert_eq!(root.introspect().tokens_after(&at(&[0]), 0), ["!", ";"]);
}

#[test]
fn get_next_tokens_with_no_parent_returns_empty() {
    // Expression.test.ts 'getNextTokens with no parent returns empty'
    assert!(options().next_tokens(&PatternPath::root()).is_empty());
}

#[test]
fn get_next_tokens_with_parent_returns_parents_tokens_after() {
    // Expression.test.ts 'getNextTokens with parent returns parent's tokens
    // after'
    let root = wrapped(expression(vec![variable()]));
    assert_eq!(root.next_tokens(&at(&[0])), [";"]);
}

#[test]
fn get_next_patterns_with_no_parent_returns_empty() {
    // Expression.test.ts 'getNextPatterns with no parent returns empty'
    assert!(options().next_patterns(&PatternPath::root()).is_empty());
}

#[test]
fn get_next_patterns_with_parent_returns_parents_patterns_after() {
    // Expression.test.ts 'getNextPatterns with parent returns parent's
    // patterns after'
    let root = wrapped(expression(vec![variable()]));
    assert_eq!(names(&root.next_patterns(&at(&[0]))), ["end"]);
}

#[test]
fn find_delegates_to_find_pattern() {
    // Expression.test.ts 'find delegates to findPattern'
    let root = options();
    let found = root.find(|p| p.name() == "a").unwrap();

    assert_eq!(found.name(), "a");
    assert_eq!(found.path, at(&[0]));
    assert!(root.find(|p| p.name() == "nonexistent").is_none());
}

#[test]
fn find_searches_the_classified_children() {
    // v12 searches a built expression's `children`: operators, not the
    // alternatives they came from, and no self-references.
    let root = expression(vec![add(), variable()]);

    assert_eq!(root.find(|p| p.name() == "plus").unwrap().path, at(&[0, 0]));
    let operator = root.find(|p| p.name() == "add-delimiter").unwrap();
    assert_eq!(operator.path, at(&[0]));
    // The self-references are no children of the operator.
    assert_eq!(
        root.find(|p| p.type_name() == "reference").map(|h| h.path),
        None
    );
}

#[test]
fn get_patterns_after_for_unknown_child_returns_empty() {
    // Expression.test.ts 'getPatternsAfter for unknown child returns empty'
    assert!(options().patterns_after(3).is_empty());
}

#[test]
fn is_equal_compares_two_expressions() {
    // Expression.test.ts 'isEqual compares two expressions'
    let a = choice("expression", vec![lit("a", "a"), lit("b", "b")]);
    let b = a.clone_named("expression");
    let c = choice("expression", vec![lit("a", "a"), lit("c", "c")]);

    assert!(a.is_equal(&b));
    assert!(!a.is_equal(&c));
}

#[test]
fn ordered_get_tokens_and_get_patterns() {
    // Expression.test.ts (ordered choice) 'Get Tokens and Get Patterns'
    let root = choice("a-or-b", vec![lit("a", "A"), lit("b", "B")]);

    assert_eq!(root.tokens(), ["A", "B"]);
    assert_eq!(names(&root.patterns()), ["a", "b"]);
}

#[test]
fn ordered_properties() {
    // Expression.test.ts (ordered choice) 'Properties'
    let root = choice("a", vec![lit("a", "A")]);

    assert_eq!(root.type_name(), "expression");
    assert_eq!(root.name(), "a");
    assert_eq!(root.introspect().at(&at(&[0])).unwrap().name(), "a");
}

#[test]
fn ordered_tokens_and_patterns_after_an_alternative_are_what_follows() {
    // Expression.test.ts (ordered choice) 'Tokens and patterns after an
    // alternative are what follows the expression'
    let root = seq(
        "sequence",
        vec![
            choice("a-or-b", vec![lit("a", "A"), lit("b", "B")]),
            lit("c", "C"),
        ],
    );
    let intro = root.introspect();

    assert_eq!(intro.tokens_after(&at(&[0]), 0), ["C"]);
    assert_eq!(names(&intro.patterns_after(&at(&[0]), 0)), ["c"]);
    assert_eq!(intro.next_tokens(&at(&[0, 0])), ["C"]);
    assert_eq!(intro.next_tokens(&at(&[0])), ["C"]);
    assert_eq!(names(&intro.next_patterns(&at(&[0]))), ["c"]);
}

#[test]
fn ordered_tokens_and_patterns_after_with_no_parent_are_empty() {
    // Expression.test.ts (ordered choice) 'Tokens and patterns after, with
    // no parent, are empty'
    let root = choice("a-or-b", vec![lit("a", "A"), lit("b", "B")]);

    assert!(root.tokens_after(0).is_empty());
    assert!(root.patterns_after(0).is_empty());
    assert!(root.next_tokens(&PatternPath::root()).is_empty());
    assert!(root.next_patterns(&PatternPath::root()).is_empty());
}

#[test]
fn mutually_recursive_choices_tokens_terminate() {
    // Expression.test.ts 'Mutually recursive choices parse, and their tokens
    // terminate' (the lesson: without the atom skip, `a = b | "c"`,
    // `b = a | "d"` overflows the stack). v12 offers each level's literals
    // until the atom being expanded twice is skipped:
    // `["c", "d", "c", "d", "c"]`.
    let b = choice("b", vec![reference("a"), lit("d", "d")]);
    let mut a = choice("a", vec![b, lit("c", "c")]);

    assert_eq!(a.tokens(), ["c", "d", "c", "d", "c"]);
    assert_eq!(names(&a.patterns()), ["c", "d", "c", "d", "c"]);
    assert!(a.test("c") && a.test("d") && !a.test("x"));
}

#[test]
fn a_self_referencing_choice_alternative_is_an_atom() {
    // Expression.test.ts 'A self-referencing choice alternative is an atom,
    // not an operator': `b` is child 0 of `a`, not a postfix.
    let b = choice("b", vec![reference("a"), lit("d", "d")]);
    let a = choice("a", vec![b, lit("c", "c")]);
    let intro = a.introspect();

    assert_eq!(
        intro.at(&at(&[0])).unwrap().pattern.type_name(),
        "expression"
    );
    assert_eq!(intro.at(&at(&[1])).unwrap().name(), "c");
    // After an atom, no postfix or infix operators, and no parent.
    assert!(a.tokens_after(0).is_empty());
}

#[test]
fn continuation_tokens_after_an_operand_include_what_follows_the_expression() {
    // Expression.test.ts 'Expression: continuation tokens': `expr = expr "+"
    // expr | /[a-z]/`, `stmt = expr ";"`. After an operand: an operator, or
    // the end of the expression. v12: `["+", ";"]`.
    let statement = seq(
        "stmt",
        vec![
            choice(
                "expr",
                vec![
                    seq(
                        "add",
                        vec![reference("expr"), lit("plus", "+"), reference("expr")],
                    ),
                    re_tokens("atom", "[a-z]", &["x"]),
                ],
            ),
            lit("semi", ";"),
        ],
    );
    let intro = statement.introspect();

    assert_eq!(intro.tokens_after(&at(&[0]), 1), ["+", ";"]);
    assert_eq!(intro.next_tokens(&at(&[0, 1])), ["+", ";"]);
    // After the operator, an operand.
    assert_eq!(intro.next_tokens(&at(&[0, 0, 0])), ["x"]);
}

#[test]
fn operators_reached_through_references_are_classified_by_their_targets() {
    // `expr = add | neg | num`, each a reference: classification resolves
    // them one level (DESIGN §8). Checked against v12.
    let root = context(
        "defs",
        seq("stmt", vec![reference("expr"), lit("semi", ";")]),
        vec![
            choice(
                "expr",
                vec![reference("add"), reference("neg"), reference("num")],
            ),
            add_named("add", "expr"),
            seq("neg", vec![lit("minus", "-"), reference("expr")]),
            re_tokens("num", "[0-9]+", &["0"]),
        ],
    );
    let intro = root.introspect();
    let expr = at(&[0, 0, 0]);

    assert_eq!(intro.tokens(&expr), ["-", "0"]);
    let children: Vec<_> = (0..3)
        .map(|i| intro.at(&at(&[0, 0, 0, i])).unwrap().name())
        .collect();
    assert_eq!(children, ["add-delimiter", "neg-prefix", "num"]);
    assert_eq!(intro.next_tokens(&at(&[0, 0, 0, 2])), ["+", ";"]);
}

fn add_named(name: &str, expression: &str) -> Pattern {
    seq(
        name,
        vec![
            reference(expression),
            lit("plus", "+"),
            reference(expression),
        ],
    )
}

// ─── RightAssociated.test.ts ───

#[test]
fn right_associated_metadata() {
    // RightAssociated.test.ts 'metadata: type and name'
    let marker = right(lit("x", "x"));

    assert_eq!(marker.type_name(), "right-associated");
    assert_eq!(marker.name(), "");
}

#[test]
fn right_associated_clone() {
    // RightAssociated.test.ts 'clone creates an independent copy'
    let marker = right(lit("x", "x"));
    let clone = marker.clone_named("ignored");

    assert_eq!(clone.type_name(), "right-associated");
    assert_eq!(clone.id(), marker.id());
    assert!(clone.is_equal(&marker));
}

#[test]
fn right_associated_get_tokens_delegates_to_child() {
    // RightAssociated.test.ts 'getTokens delegates to child'
    assert_eq!(right(lit("x", "x")).tokens(), ["x"]);
}

#[test]
fn right_associated_get_patterns_delegates_to_child() {
    // RightAssociated.test.ts 'getPatterns delegates to child'
    assert_eq!(names(&right(lit("x", "x")).patterns()), ["x"]);
}

#[test]
fn right_associated_find_searches_within_child() {
    // RightAssociated.test.ts 'find searches within child'
    let marker = right(seq("ab", vec![lit("a", "a"), lit("b", "b")]));
    let found = marker.find(|p| p.name() == "b").unwrap();

    assert_eq!(found.name(), "b");
    // The marker is no step of a path.
    assert_eq!(found.path, at(&[1]));
    assert!(marker.find(|p| p.name() == "z").is_none());
}

#[test]
fn right_associated_is_equal_compares_structure() {
    // RightAssociated.test.ts 'isEqual compares structure'
    let a = right(lit("x", "x"));
    let b = right(lit("x", "x"));
    let c = right(lit("y", "y"));

    assert!(a.is_equal(&b));
    assert!(!a.is_equal(&c));
    assert!(!a.is_equal(&lit("x", "x")), "a marker is no literal");
}

#[test]
fn right_associated_standalone_next_is_empty() {
    // RightAssociated.test.ts 'standalone getNextTokens/getNextPatterns
    // return empty (no parent)' and 'getTokensAfter and getPatternsAfter
    // return empty when no parent'
    let marker = right(lit("x", "x"));

    assert!(marker.next_tokens(&PatternPath::root()).is_empty());
    assert!(marker.next_patterns(&PatternPath::root()).is_empty());
    assert!(marker.tokens_after(0).is_empty());
    assert!(marker.patterns_after(0).is_empty());
}

#[test]
fn right_associated_introspection_delegates_to_the_parent() {
    // RightAssociated.test.ts 'introspection delegates to parent when parent
    // exists'. The marker is no step, so `[0]` is the marked pattern, and
    // what follows it is what follows the marker.
    let statement = seq("stmt", vec![right(lit("x", "x")), lit("semi", ";")]);

    assert_eq!(statement.next_tokens(&at(&[0])), [";"]);
    assert_eq!(statement.next_patterns(&at(&[0])).len(), 1);
    assert_eq!(statement.introspect().tokens_after(&at(&[]), 0), [";"]);
    assert_eq!(statement.introspect().patterns_after(&at(&[]), 0).len(), 1);
}

#[test]
fn a_marked_infix_is_an_operator_like_any_other() {
    // RightAssociated.test.ts 'ternary expression uses right-association in
    // an Expression', introspected: the marked ternary is an infix.
    let ternary = seq(
        "ternary",
        vec![
            reference("expression"),
            lit("question-mark", " ? "),
            reference("expression"),
            lit("colon", " : "),
            reference("expression"),
        ],
    );
    let variables = choice("variables", vec![lit("a", "a"), lit("b", "b")]);
    let root = choice("expression", vec![right(ternary), variables]);
    let intro = root.introspect();

    assert_eq!(intro.at(&at(&[0])).unwrap().name(), "ternary-delimiter");
    assert_eq!(root.tokens_after(1), [" ? "]);
    assert_eq!(intro.next_tokens(&at(&[0, 0])), ["a", "b"]);
    assert_eq!(intro.next_tokens(&at(&[0, 1])), [" : "]);
}

#[test]
fn an_operator_from_an_implicit_target_resolves_its_parts_at_home() {
    // `e = op | "x"`, where `op = e "+" u` is defined inside context `k`,
    // which defines `u`: the operator's part `"+" u` resolves `u` where
    // `op` is defined (DESIGN §11, §12).
    let root = seq(
        "root",
        vec![
            choice("e", vec![reference("op"), lit("x", "x")]),
            context(
                "k",
                seq(
                    "k-body",
                    vec![seq(
                        "op",
                        vec![reference("e"), lit("plus", "+"), reference("u")],
                    )],
                ),
                vec![lit("u", "U")],
            ),
        ],
    );
    let intro = root.introspect();

    assert_eq!(intro.at(&at(&[0, 0])).unwrap().name(), "op-postfix");
    assert_eq!(intro.next_tokens(&at(&[0, 0, 0])), ["U"]);
}

// ─── Roles (v12's Expression accessors) ───

fn roles_at(root: &Pattern, path: &[u32]) -> Option<Vec<cpat_core::ChoiceRole>> {
    root.introspect().choice_roles(&PatternPath::new(path))
}

fn names_at(root: &Pattern, path: &[u32], count: usize) -> Vec<String> {
    (0..count as u32)
        .map(|i| {
            let mut child = path.to_vec();
            child.push(i);
            root.introspect()
                .at(&PatternPath::new(&child))
                .unwrap()
                .name()
                .to_string()
        })
        .collect()
}

#[test]
fn roles_partition_the_children_as_v12_classifies_them() {
    // Expression.test.ts 'Accessor: infixPatterns, prefixPatterns,
    // postfixPatterns': one of each, named as v12 names them (checked by
    // running v12's build() on the same grammar).
    use cpat_core::ChoiceRole::*;
    let root = expression(vec![add(), negate(), factorial(), variable()]);

    assert_eq!(
        roles_at(&root, &[]),
        Some(vec![Infix, Prefix, Postfix, Atom])
    );
    assert_eq!(
        names_at(&root, &[], 4),
        [
            "add-delimiter",
            "negate-prefix",
            "factorial-postfix",
            "variable"
        ]
    );
}

#[test]
fn roles_agree_with_the_choices_own_accessors() {
    let Pattern::Choice(choice) = expression(vec![add(), negate(), factorial(), variable()]) else {
        unreachable!()
    };
    let names = |it: Vec<&str>| it.into_iter().map(str::to_string).collect::<Vec<_>>();
    assert_eq!(
        names(choice.infix_patterns().map(|s| s.name()).collect()),
        ["add-delimiter"]
    );
    assert_eq!(
        names(choice.prefix_patterns().map(|s| s.name()).collect()),
        ["negate-prefix"]
    );
    assert_eq!(
        names(choice.postfix_patterns().map(|s| s.name()).collect()),
        ["factorial-postfix"]
    );
    assert_eq!(choice.atom_patterns().count(), 1);
}

#[test]
fn a_choice_waiting_for_its_first_parse_is_classified_on_demand() {
    // v12's build(): a reference alternative is resolved to see whether it
    // is an operator, before anything has parsed.
    use cpat_core::ChoiceRole::*;
    let root = context(
        "root",
        expression(vec![reference("add"), variable()]),
        vec![add()],
    );

    assert_eq!(roles_at(&root, &[0]), Some(vec![Infix, Atom]));
}

#[test]
fn only_a_choice_has_roles() {
    assert_eq!(roles_at(&wrapped(options()), &[]), None);
    assert_eq!(
        roles_at(&wrapped(options()), &[0]),
        Some(vec![cpat_core::ChoiceRole::Atom; 3])
    );
    assert_eq!(roles_at(&options(), &[7]), None);
}

// ─── v12's build(): a reference alternative that names nothing ───
// Each answer below is v12's, from running `build()` on the same pattern.

fn check_alternatives_at(root: &Pattern, path: &[u32]) -> Result<(), String> {
    root.introspect()
        .check_alternatives(&PatternPath::new(path))
        .map_err(|error| match error {
            cpat_core::PatternError::UnknownReference { target, .. } => target,
            other => other.to_string(),
        })
}

#[test]
fn check_alternatives_reports_a_reference_alternative_that_names_nothing() {
    // v12: `new Expression("x", [new Reference("nope"), n]).build()` throws
    // "Couldn't find 'nope' pattern within tree."
    let root = choice("x", vec![reference("nope"), lit("n", "1")]);

    assert_eq!(check_alternatives_at(&root, &[]), Err("nope".to_string()));
}

#[test]
fn check_alternatives_sees_through_right_association() {
    let root = choice("x", vec![right(reference("nope")), lit("n", "1")]);

    assert_eq!(check_alternatives_at(&root, &[]), Err("nope".to_string()));
}

#[test]
fn check_alternatives_leaves_deeper_references_to_the_parse() {
    // v12's build() resolves only the alternatives themselves: these two
    // build without a throw (their exec throws).
    let in_sequence = choice(
        "x",
        vec![
            seq("s", vec![lit("a", "a"), reference("nope")]),
            lit("n", "1"),
        ],
    );
    let in_operator = choice(
        "x",
        vec![
            seq("p", vec![lit("m", "-"), reference("x"), reference("nope")]),
            lit("n", "1"),
        ],
    );

    assert_eq!(check_alternatives_at(&in_sequence, &[]), Ok(()));
    assert_eq!(check_alternatives_at(&in_operator, &[]), Ok(()));
    assert!(in_sequence.check().is_err());
}

#[test]
fn check_alternatives_resolves_from_the_place_as_a_parse_would() {
    // v12: a child expression's reference finds a sibling through its
    // parent, and finds the expression itself.
    let sibling = seq(
        "s",
        vec![
            choice("x", vec![reference("y"), lit("n", "1")]),
            lit("y", "y"),
        ],
    );
    let missing = seq(
        "s",
        vec![choice("x", vec![reference("nope"), lit("n", "1")])],
    );
    let itself = choice("x", vec![reference("x"), lit("n", "1")]);

    assert_eq!(check_alternatives_at(&sibling, &[0]), Ok(()));
    assert_eq!(
        check_alternatives_at(&missing, &[0]),
        Err("nope".to_string())
    );
    assert_eq!(check_alternatives_at(&itself, &[]), Ok(()));
}

#[test]
fn check_alternatives_is_ok_anywhere_but_a_choice() {
    let root = seq("s", vec![reference("nope")]);

    assert_eq!(check_alternatives_at(&root, &[]), Ok(()));
    assert_eq!(check_alternatives_at(&root, &[9]), Ok(()));
}
