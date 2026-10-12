//! v12's `PrecedenceTree.test.ts` scenarios, reproduced through a choice in
//! Pratt mode: the grammar's declaration order gives the precedence map the
//! v12 test hands its tree, and the input makes the same sequence of
//! `addPrefix`/`addAtom`/`addPostfix`/`addBinary` calls. Expected shapes are
//! v12's (`name:type@start..end`, from running both on the same grammar and
//! input). Dangling operators use `parse`, which keeps a match that stops
//! short of the end; `exec` would reject the leftover text.

mod common;

use common::*;
use cpat_core::Pattern;

fn e() -> Pattern {
    reference("expression")
}

fn atoms(names: &[&str]) -> Vec<Pattern> {
    names.iter().map(|n| lit(n, n)).collect()
}

/// `mul`, `add`, `bool` at levels 0, 1, 2 (v12's precedence map), then
/// `extra`, then the atoms `a`..`e`. `right_mul` wraps `mul` in
/// `RightAssociated` (v12's `{ mul: Association.right }`).
fn binary_levels(extra: Vec<Pattern>, right_mul: bool) -> Pattern {
    let mul = seq("mul", vec![e(), lit("*", "*"), e()]);
    let mut alternatives = vec![
        if right_mul { right(mul) } else { mul },
        seq("add", vec![e(), lit("+", "+"), e()]),
        seq("bool", vec![e(), lit("||", "||"), e()]),
    ];
    alternatives.extend(extra);
    alternatives.extend(atoms(&["a", "b", "c", "d", "e"]));
    choice("expression", alternatives)
}

/// The `all` scenario: `negate` and `increment` after the binary levels, so
/// the prefix (index 3) binds tighter than the postfix (index 4) as in v12's
/// test, where neither has a precedence and ties keep the prefix inside.
fn all_levels() -> Pattern {
    binary_levels(
        vec![
            seq("negate", vec![lit("!", "!"), e()]),
            seq("increment", vec![e(), lit("++", "++")]),
        ],
        false,
    )
}

#[test]
fn add_binary() {
    assert_eq!(
        exec_shape(&mut binary_levels(vec![], false), "a+b*c||d+e"),
        "bool:expression@0..10[add:expression@0..5[a:literal@0..1 +:literal@1..2 \
         mul:expression@2..5[b:literal@2..3 *:literal@3..4 c:literal@4..5]] ||:literal@5..7 \
         add:expression@7..10[d:literal@7..8 +:literal@8..9 e:literal@9..10]]"
    );
}

#[test]
fn add_prefix() {
    let mut expression = choice(
        "expression",
        vec![
            seq("negate", vec![lit("!", "!"), e()]),
            seq("increment", vec![lit("++", "++"), e()]),
            seq("plus", vec![lit("+", "+"), e()]),
            lit("a", "a"),
        ],
    );

    assert_eq!(
        exec_shape(&mut expression, "!+++a"),
        "negate:expression@0..5[!:literal@0..1 increment:expression@1..5[++:literal@1..3 \
         plus:expression@3..5[+:literal@3..4 a:literal@4..5]]]"
    );
}

#[test]
fn add_postfix() {
    let mut expression = choice(
        "expression",
        vec![
            seq("decrement", vec![e(), lit("--", "--")]),
            seq("increment", vec![e(), lit("++", "++")]),
            lit("a", "a"),
        ],
    );

    assert_eq!(
        exec_shape(&mut expression, "a--++"),
        "increment:expression@0..5[decrement:expression@0..3[a:literal@0..1 --:literal@1..3] \
         ++:literal@3..5]"
    );
}

#[test]
fn all() {
    assert_eq!(
        exec_shape(&mut all_levels(), "!a++*b+c"),
        "add:expression@0..8[mul:expression@0..6[increment:expression@0..4[\
         negate:expression@0..2[!:literal@0..1 a:literal@1..2] ++:literal@2..4] *:literal@4..5 \
         b:literal@5..6] +:literal@6..7 c:literal@7..8]"
    );
}

#[test]
fn incomplete() {
    assert_eq!(
        parse_shape(&mut all_levels(), "!a++*b+c*").0,
        "add:expression@0..8[mul:expression@0..6[increment:expression@0..4[\
         negate:expression@0..2[!:literal@0..1 a:literal@1..2] ++:literal@2..4] *:literal@4..5 \
         b:literal@5..6] +:literal@6..7 c:literal@7..8]"
    );
}

#[test]
fn add_partial_binary_with_lower_precedence() {
    assert_eq!(
        parse_shape(&mut binary_levels(vec![], false), "a+b*c||"),
        outlined(
            "add:expression@0..5[a:literal@0..1 +:literal@1..2 mul:expression@2..5[\
             b:literal@2..3 *:literal@3..4 c:literal@4..5]]",
            "-",
            "*@5..5"
        )
    );
}

#[test]
fn add_partial_binary_with_equal_precedence() {
    assert_eq!(
        parse_shape(&mut binary_levels(vec![], false), "a+b*c*").0,
        "add:expression@0..5[a:literal@0..1 +:literal@1..2 mul:expression@2..5[\
         b:literal@2..3 *:literal@3..4 c:literal@4..5]]"
    );
}

#[test]
fn add_partial_binary_with_equal_precedence_and_right_associated() {
    assert_eq!(
        parse_shape(&mut binary_levels(vec![], true), "a+b*c*").0,
        "add:expression@0..5[a:literal@0..1 +:literal@1..2 mul:expression@2..5[\
         b:literal@2..3 *:literal@3..4 c:literal@4..5]]"
    );
}

#[test]
fn mul_partial_binary_with_greater_precedence() {
    assert_eq!(
        parse_shape(&mut binary_levels(vec![], false), "a+b*").0,
        "add:expression@0..3[a:literal@0..1 +:literal@1..2 b:literal@2..3]"
    );
}

#[test]
fn a_single_binary_without_a_trailing_atom_reverts_to_the_atom() {
    // The operand alone comes back unwrapped, not as an expression node.
    assert_eq!(
        parse_shape(&mut binary_levels(vec![], false), "a+"),
        outlined("a:literal@0..1", "-", "*@1..1")
    );
}

#[test]
fn right_associated_nests_right_with_a_tighter_operator_between() {
    let mut expression = choice(
        "expression",
        [
            vec![
                seq("add", vec![e(), lit("+", "+"), e()]),
                right(seq("as", vec![e(), lit("as", "as"), e()])),
            ],
            atoms(&["a", "b", "c", "d"]),
        ]
        .concat(),
    );

    assert_eq!(
        exec_shape(&mut expression, "aasb+casd"),
        "as:expression@0..9[a:literal@0..1 as:literal@1..3 as:expression@3..9[\
         add:expression@3..6[b:literal@3..4 +:literal@4..5 c:literal@5..6] as:literal@6..8 \
         d:literal@8..9]]"
    );
}

#[test]
fn right_associated_operators_of_one_level_nest_right() {
    let mut expression = binary_levels(vec![], true);

    assert_eq!(
        parse_shape(&mut expression, "a*b*c").0,
        "mul:expression@0..5[a:literal@0..1 *:literal@1..2 mul:expression@2..5[\
         b:literal@2..3 *:literal@3..4 c:literal@4..5]]"
    );
    assert_eq!(
        parse_shape(&mut expression, "a+b*c*d").0,
        "add:expression@0..7[a:literal@0..1 +:literal@1..2 mul:expression@2..7[\
         b:literal@2..3 *:literal@3..4 mul:expression@4..7[c:literal@4..5 *:literal@5..6 \
         d:literal@6..7]]]"
    );
}

#[test]
fn a_prefix_declared_after_a_postfix_goes_outside_it() {
    // The other order of `all`: the postfix (index 0) binds tighter than the
    // prefix (index 1), so `!a++` is `!(a++)`.
    let mut expression = choice(
        "expression",
        vec![
            seq("increment", vec![e(), lit("++", "++")]),
            seq("negate", vec![lit("!", "!"), e()]),
            lit("a", "a"),
        ],
    );

    assert_eq!(
        exec_shape(&mut expression, "!a++"),
        "negate:expression@0..4[!:literal@0..1 increment:expression@1..4[a:literal@1..2 \
         ++:literal@2..4]]"
    );
    assert_eq!(
        exec_shape(&mut expression, "!!a++++"),
        "negate:expression@0..7[!:literal@0..1 negate:expression@1..7[!:literal@1..2 \
         increment:expression@2..7[increment:expression@2..5[a:literal@2..3 ++:literal@3..5] \
         ++:literal@5..7]]]"
    );
    // And in declaration order `all`'s way round: `(!a)++`.
    assert_eq!(
        exec_shape(&mut all_levels(), "!a++"),
        "increment:expression@0..4[negate:expression@0..2[!:literal@0..1 a:literal@1..2] \
         ++:literal@2..4]"
    );
}
