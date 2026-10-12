//! `generate_error_message`, ported from v12 `generate_error_message.ts`.
//! Every expected string was printed by v12's `generateErrorMessage` on the
//! same pattern and input (the source test or the v12 run is named on each).

mod common;

use common::*;
use cpat_core::{Choice, Pattern, generate_error_message};

fn message(mut pattern: Pattern, text: &str) -> String {
    let result = pattern.exec(text);
    generate_error_message(&pattern, &result)
}

fn full_name() -> Pattern {
    seq(
        "full-name",
        vec![
            lit("first", "John"),
            re("space", r"\s+"),
            lit("last", "Doe"),
        ],
    )
}

#[test]
fn the_error_message_is_unchanged() {
    // BasePattern.test.ts 'exec keeps the real furthest error' › 'the error
    // message is unchanged'. The leaf match is `space` (ending at 5); v12
    // measures one unit past it, so the column is 6.
    assert_eq!(
        message(full_name(), "John Dot"),
        "Error at line 1, column 6. Hint: Doe"
    );
}

#[test]
fn the_error_message_through_a_context_is_unchanged() {
    // v12 on `Context("names", fullName)`: the same message, the leaf
    // match's path going through the context's child.
    let root = context("names", full_name(), vec![]);
    assert_eq!(
        message(root, "John Dot"),
        "Error at line 1, column 6. Hint: Doe"
    );
}

#[test]
fn nothing_matched_hints_what_starts_the_pattern_joined_with_a_comma_and_space() {
    // v12 on full-name: no leaf match, so line 1, column 1 and the root's
    // tokens, joined with ", ".
    assert_eq!(
        message(full_name(), ""),
        "Error at line 1, column 1. Hint: John"
    );
    assert_eq!(
        message(full_name(), "Jo"),
        "Error at line 1, column 1. Hint: John"
    );
    let words = choice("w", vec![lit("a", "a"), lit("b", "b"), lit("c", "c")]);
    assert_eq!(
        message(words, "x"),
        "Error at line 1, column 1. Hint: a, b, c"
    );
}

#[test]
fn after_a_match_the_hint_joins_with_a_bare_comma() {
    // v12 prints the array in a template string: `","` between items. The
    // continuation grammar after `a`: an operator or what follows.
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
    assert_eq!(
        message(statement, "a"),
        "Error at line 1, column 1. Hint: +,;"
    );
}

#[test]
fn a_complete_match_still_gets_a_message() {
    // v12 on full-name "John Doe": the leaf ends the text, so the measured
    // text stops there; nothing follows `last`.
    assert_eq!(
        message(full_name(), "John Doe"),
        "Error at line 1, column 8. Hint: "
    );
}

#[test]
fn suggestions_are_trimmed_and_blanks_dropped() {
    // v12 on `s = "a" " b "`: the suggestion is ` b `, trimmed.
    let pattern = seq("s", vec![lit("a", "a"), lit("b", " b ")]);
    assert_eq!(message(pattern, "ab"), "Error at line 1, column 2. Hint: b");

    // A suggestion that is only whitespace disappears.
    let lines = seq("lines", vec![lit("a", "a"), lit("nl", "\n")]);
    assert_eq!(message(lines, "ax"), "Error at line 1, column 2. Hint: ");
}

/// `"a\n" "😀" "b" "\n" "c"`.
fn lines() -> Pattern {
    seq(
        "lines",
        vec![
            lit("first", "a\n"),
            lit("emoji", "😀"),
            lit("last", "b"),
            lit("nl", "\n"),
            lit("tail", "c"),
        ],
    )
}

#[test]
fn a_multi_line_input_counts_lines_and_utf16_columns() {
    // v12 on `lines`. Columns count UTF-16 units, so the emoji is two.
    assert_eq!(
        message(lines(), "a\nX"),
        "Error at line 2, column 1. Hint: 😀"
    );
    assert_eq!(
        message(lines(), "a\n😀c"),
        "Error at line 2, column 3. Hint: b"
    );
    assert_eq!(
        message(lines(), "a\n😀b\nd"),
        "Error at line 3, column 1. Hint: c"
    );
}

#[test]
fn a_newline_right_after_the_match_starts_a_new_line_at_column_0() {
    // v12 on `"a" " b "` with "a\nb": the measured text is "a\n".
    let pattern = seq("s", vec![lit("a", "a"), lit("b", " b ")]);
    assert_eq!(
        message(pattern, "a\nb"),
        "Error at line 2, column 0. Hint: b"
    );
}

#[test]
fn an_emoji_before_the_error_counts_two_units() {
    // v12 on `"😀" "x" "y"`.
    let emoji = || {
        seq(
            "emoji",
            vec![lit("smile", "😀"), lit("x", "x"), lit("y", "y")],
        )
    };

    assert_eq!(
        message(emoji(), "😀z"),
        "Error at line 1, column 3. Hint: x"
    );
    assert_eq!(
        message(emoji(), "😀xz"),
        "Error at line 1, column 4. Hint: y"
    );
}

#[test]
fn an_emoji_right_after_the_match_counts_one_unit() {
    // v12 on `"a" " b "` with "a😀": it measures one UTF-16 unit past the
    // match, half the emoji.
    let pattern = seq("s", vec![lit("a", "a"), lit("b", " b ")]);
    assert_eq!(
        message(pattern, "a😀"),
        "Error at line 1, column 2. Hint: b"
    );
}

const KEYWORDS: [&str; 11] = [
    "if", "in", "int", "else", "elif", "for", "fo", "while", "x", "é", "😀a",
];

fn keyword_choice() -> Pattern {
    let literals = KEYWORDS
        .iter()
        .map(|k| lit(&format!("kw-{k}"), k))
        .collect();
    choice("keyword", literals)
}

fn ordered_keywords() -> Pattern {
    let literals = KEYWORDS
        .iter()
        .map(|k| lit(&format!("kw-{k}"), k))
        .collect();
    Choice::ordered("keyword", literals).unwrap().into()
}

#[test]
fn the_literal_set_message_matches_the_plain_loop() {
    // Expression.test.ts 'Expression: literal-set fast path' › 'results,
    // furthest error and error message match the plain loop' (the message
    // part; the rest is in literal_set.rs). v12 compares with a recording
    // parse; both are compared here, and with the ordered choice.
    let inputs = [
        "",
        "i",
        "if",
        "in",
        "int",
        "inx",
        "el",
        "els",
        "elif",
        "else",
        "elsewhere",
        "f",
        "fo",
        "for",
        "forx",
        "w",
        "whil",
        "x",
        "y",
        "é",
        "😀",
        "😀a",
        "😀b",
    ];
    for text in inputs {
        let fast = message(keyword_choice(), text);
        let mut recorded = keyword_choice();
        let result = recorded.exec_recording(text);
        assert_eq!(fast, generate_error_message(&recorded, &result), "{text:?}");
        assert_eq!(fast, message(ordered_keywords(), text), "{text:?}");
    }
    assert_eq!(
        message(keyword_choice(), "y"),
        "Error at line 1, column 1. Hint: if, in, int, else, elif, for, fo, while, x, é, 😀a"
    );
}

#[test]
fn inside_a_sequence_the_literal_set_message_matches_the_plain_loop() {
    // Expression.test.ts 'Expression: literal-set fast path' › 'inside a
    // sequence, the furthest error and message match the plain loop'.
    // Messages printed by v12 for `stmt = keyword ";"`.
    let statement = |keywords: Pattern| seq("stmt", vec![keywords, lit("semi", ";")]);
    let expected = [
        ("int;", "Error at line 1, column 3. Hint: ;"),
        ("inx;", "Error at line 1, column 3. Hint: ;"),
        (
            "els;",
            "Error at line 1, column 1. Hint: if, in, int, else, elif, for, fo, while, x, é, 😀a",
        ),
        ("elif", "Error at line 1, column 4. Hint: ;"),
        (
            "zz;",
            "Error at line 1, column 1. Hint: if, in, int, else, elif, for, fo, while, x, é, 😀a",
        ),
        ("for;;", "Error at line 1, column 5. Hint: "),
    ];
    for (text, v12) in expected {
        assert_eq!(message(statement(keyword_choice()), text), v12, "{text:?}");
        assert_eq!(
            message(statement(ordered_keywords()), text),
            v12,
            "{text:?}"
        );
    }
}

#[test]
fn an_expression_message_offers_the_operators_or_the_close() {
    // v12 on `Expression.test.ts`'s `createExpressionPattern`, after "(a":
    // every infix operator's first token, the ternary's `?`, and the
    // group's `)` (the optional spaces' token is blank, so dropped).
    let spaces = || opt("optional-spaces", re_tokens("spaces", r"\s+", &[" "]));
    let binary = |name: &str, operators: Pattern| {
        seq(
            name,
            vec![reference("expression"), operators, reference("expression")],
        )
    };
    let root = choice(
        "expression",
        vec![
            binary(
                "mult-div-expression",
                choice("mult-div", vec![lit("mult", " * "), lit("div", " / ")]),
            ),
            binary(
                "add-sub-expression",
                choice("add-sub", vec![lit("add", " + "), lit("sub", " - ")]),
            ),
            binary(
                "bool-expression",
                choice("and-or", vec![lit("and", " && "), lit("or", " || ")]),
            ),
            right(seq(
                "ternary",
                vec![
                    reference("expression"),
                    spaces(),
                    lit("question-mark", "?"),
                    spaces(),
                    reference("expression"),
                    spaces(),
                    lit("colon", ":"),
                    spaces(),
                    reference("expression"),
                ],
            )),
            seq(
                "group",
                vec![
                    lit("open-paren", "("),
                    spaces(),
                    reference("expression"),
                    spaces(),
                    lit("close-paren", ")"),
                ],
            ),
            choice(
                "variables",
                ["a", "b", "c", "d", "e"]
                    .iter()
                    .map(|v| lit(v, v))
                    .collect(),
            ),
        ],
    );
    assert_eq!(
        message(root.clone(), "(a"),
        "Error at line 1, column 2. Hint: *,/,+,-,&&,||,?,)"
    );
    assert_eq!(
        message(root.clone(), "a + "),
        "Error at line 1, column 4. Hint: (,a,b,c,d,e"
    );
    assert_eq!(
        message(root, ""),
        "Error at line 1, column 1. Hint: (, a, b, c, d, e"
    );
}

#[test]
fn a_leaf_match_ending_at_the_start_reads_as_no_match() {
    // v12's `endIndex === 0` branch: line 1, column 1 and what starts the
    // root. Only custom code can record a match that ends at 0.
    use cpat_core::{Cursor, Custom, CustomParse, Meta, NodeId};

    #[derive(Clone)]
    struct EmptyThenFail;
    impl CustomParse for EmptyThenFail {
        fn parse(&mut self, meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
            cursor.record_match(meta, 0, 0);
            cursor.record_error(0, 0, meta);
            None
        }
        fn clone_box(&self) -> Box<dyn CustomParse> {
            Box::new(self.clone())
        }
    }

    let mut custom: Pattern = Custom::new("c", Box::new(EmptyThenFail)).into();
    custom
        .tokenizable_mut()
        .unwrap()
        .set_tokens(vec!["c".into()]);
    let root = seq("s", vec![custom, lit("b", "B")]);

    assert_eq!(message(root, "x"), "Error at line 1, column 1. Hint: c");
}
