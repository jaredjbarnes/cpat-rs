//! Ports of v12 `grammar/Grammar.test.ts`: every test that uses no
//! imports, params or decorators (those arrive with later milestones), and
//! v12's `grammar/patterns.test.ts`. Error tests assert v12's message text
//! exactly. Positions are byte offsets; the texts are ASCII.
//!
//! Not ported: 'patterns template converts kebab-case to camelCase' (a JS
//! template-literal helper with no Rust counterpart).

mod common;

use common::*;
use cpat_core::{
    Context, Cursor, Grammar, GrammarError, GrammarOptions, NodeKind, Pattern, PatternError,
    Patterns, Repeat, RepeatOptions, Tree,
};

fn build(text: &str) -> Patterns {
    Grammar::parse_string(text, GrammarOptions::default()).expect("the grammar builds")
}

fn build_error(text: &str) -> GrammarError {
    Grammar::parse_string(text, GrammarOptions::default()).expect_err("the grammar is refused")
}

/// The message of the error building `text` gives.
fn message(text: &str) -> String {
    build_error(text).to_string()
}

/// The value `rule` matches when it runs over all of `text`.
fn value(patterns: &mut Patterns, rule: &str, text: &str) -> Option<String> {
    let result = patterns[rule].exec(text);
    result.tree().map(|tree| tree.root().value().to_string())
}

fn wrapped(name: &str, child: Pattern, patterns: Vec<Pattern>) -> Pattern {
    Context::new(name, child, patterns).into()
}

/// `name` = `node.name` children recursively, as v12's `nodeNames`.
fn node_names(tree: &Tree<'_>) -> Vec<String> {
    let mut names = Vec::new();
    let mut pending = vec![tree.root()];
    while let Some(node) = pending.pop() {
        names.push(node.name().to_string());
        pending.extend(node.children().rev());
    }
    names
}

/// v12's `patternNames`: a pattern's name, then its children's, depth
/// first. A context's child, a choice's alternatives.
fn pattern_names(pattern: &Pattern) -> Vec<String> {
    let mut names = vec![pattern.name().to_string()];
    let children = match pattern {
        Pattern::Context(context) => vec![context.child()],
        Pattern::Sequence(sequence) => sequence.children().iter().collect(),
        Pattern::Choice(choice) => choice.original_alternatives().iter().collect(),
        Pattern::Optional(optional) => vec![optional.child()],
        Pattern::Not(not) => vec![not.child()],
        Pattern::Repeat(repeat) => vec![repeat.child()],
        _ => Vec::new(),
    };
    for child in children {
        names.extend(pattern_names(child));
    }
    names
}

// ─── Patterns ───────────────────────────────────────────────────────────

#[test]
fn literal() {
    let patterns = build(
        r#"
            name = "John"
        "#,
    );
    let expected = wrapped("name", lit("name", "John"), vec![]);
    assert!(patterns["name"].is_equal(&expected));
}

#[test]
fn literal_with_escaped_characters() {
    let patterns = build(
        r#"
            chars = "\n\r\t\b\f\v\0\x00\u0000\"\\"
        "#,
    );
    let expected = wrapped(
        "chars",
        lit("chars", "\n\r\t\u{8}\u{c}\u{b}\0\0\0\"\\"),
        vec![],
    );
    assert!(patterns["chars"].is_equal(&expected));
}

#[test]
fn literal_with_escaped_quotes() {
    let patterns = build(
        r#"
            content = "With Con\"tent"
        "#,
    );
    let expected = wrapped("content", lit("content", "With Con\"tent"), vec![]);
    assert!(patterns["content"].is_equal(&expected));
}

#[test]
fn regex() {
    let patterns = build(
        r"
            name = /\w/
        ",
    );
    let expected = wrapped("name", re("name", r"\w"), vec![]);
    assert!(patterns["name"].is_equal(&expected));
}

// ─── Literal escape decoding ────────────────────────────────────────────

/// The token the cpat literal `literal` decodes to.
fn decode(literal: &str) -> String {
    let patterns = build(&format!("x = {literal}"));
    let Pattern::Context(context) = &patterns["x"] else {
        panic!("every rule is a context");
    };
    let Pattern::Literal(literal) = context.child() else {
        panic!("a literal rule builds a literal");
    };
    literal.token().to_string()
}

#[test]
fn an_escaped_backslash_followed_by_n_is_a_backslash_and_an_n() {
    assert_eq!(decode(r#""\\n""#), "\\n");
}

#[test]
fn two_escaped_backslashes_are_two_backslashes() {
    assert_eq!(decode(r#""\\\\""#), "\\\\");
}

#[test]
fn an_escaped_backslash_then_an_escaped_quote() {
    assert_eq!(decode(r#""\\\"""#), "\\\"");
}

#[test]
fn each_documented_escape() {
    assert_eq!(decode(r#""\n\r\t\b\f\v\0""#), "\n\r\t\u{8}\u{c}\u{b}\0");
    assert_eq!(decode(r#""\x41\x7a""#), "Az");
    assert_eq!(decode(r#""\u00e9\u2603""#), "\u{e9}\u{2603}");
    assert_eq!(decode(r#""\"quoted\"""#), "\"quoted\"");
}

#[test]
fn mixed_sequences_decode_left_to_right() {
    assert_eq!(decode(r#""a\\tb\tc\\\nd""#), "a\\tb\tc\\\nd");
    assert_eq!(decode(r#""\\x41\x41""#), "\\x41A");
    assert_eq!(decode(r#""\\u0041\u0041""#), "\\u0041A");
}

#[test]
fn any_other_escaped_character_means_itself() {
    assert_eq!(decode(r#""\q\/\-""#), "q/-");
}

#[test]
fn an_incomplete_x_or_u_escape_means_the_letter_itself() {
    assert_eq!(decode(r#""\xZZ""#), "xZZ");
    assert_eq!(decode(r#""\u12""#), "u12");
}

// ─── Composition ────────────────────────────────────────────────────────

#[test]
fn or() {
    let patterns = build(
        r#"
            john = "John"
            jane = "Jane"
            names = john | jane
        "#,
    );
    let (john, jane) = (lit("john", "John"), lit("jane", "Jane"));
    let names = choice("names", vec![john.clone(), jane.clone()]);
    let expected = wrapped("names", names, vec![john, jane]);
    assert!(patterns["names"].is_equal(&expected));
}

#[test]
fn and() {
    let patterns = build(
        r#"
            space = " "
            first-name = /\w/
            last-name = /\w/
            full-name = first-name + space + last-name
        "#,
    );
    let space = lit("space", " ");
    let first_name = re("first-name", r"\w");
    let last_name = re("last-name", r"\w");
    let full_name = seq(
        "full-name",
        vec![first_name.clone(), space.clone(), last_name.clone()],
    );
    let expected = wrapped("full-name", full_name, vec![space, first_name, last_name]);
    assert!(patterns["full-name"].is_equal(&expected));
}

#[test]
fn and_with_optional_pattern() {
    let patterns = build(
        r#"
            space = " "
            first-name = /\w/
            last-name = /\w/
            middle-name = /\w/
            middle-name-with-space = middle-name + space
            full-name = first-name + space + middle-name-with-space? + last-name
        "#,
    );
    let space = lit("space", " ");
    let first_name = re("first-name", r"\w");
    let last_name = re("last-name", r"\w");
    let middle_name = re("middle-name", r"\w");
    let middle_name_with_space = seq(
        "middle-name-with-space",
        vec![middle_name.clone(), space.clone()],
    );
    let full_name = seq(
        "full-name",
        vec![
            first_name.clone(),
            space.clone(),
            opt(
                "optional-middle-name-with-space",
                middle_name_with_space.clone(),
            ),
            last_name.clone(),
        ],
    );
    let expected = wrapped(
        "full-name",
        full_name,
        vec![
            space,
            first_name,
            last_name,
            middle_name,
            middle_name_with_space,
        ],
    );
    assert!(patterns["full-name"].is_equal(&expected));
}

#[test]
fn and_with_not_pattern() {
    let patterns = build(
        r#"
            space = " "
            first-name = /\w/
            last-name = /\w/
            middle-name = /\w/
            jack = "Jack"
            middle-name-with-space = middle-name + space
            full-name = !jack + first-name + space + middle-name-with-space? + last-name
        "#,
    );
    let space = lit("space", " ");
    let first_name = re("first-name", r"\w");
    let last_name = re("last-name", r"\w");
    let middle_name = re("middle-name", r"\w");
    let jack = lit("jack", "Jack");
    let middle_name_with_space = seq(
        "middle-name-with-space",
        vec![middle_name.clone(), space.clone()],
    );
    let full_name = seq(
        "full-name",
        vec![
            not("not-jack", jack.clone()),
            first_name.clone(),
            space.clone(),
            opt(
                "optional-middle-name-with-space",
                middle_name_with_space.clone(),
            ),
            last_name.clone(),
        ],
    );
    let expected = wrapped(
        "full-name",
        full_name,
        vec![
            space,
            first_name,
            last_name,
            middle_name,
            jack,
            middle_name_with_space,
        ],
    );
    assert!(patterns["full-name"].is_equal(&expected));

    // The names v12 gives the parts: `not-<inner>`, `optional-<inner>`.
    let Pattern::Context(context) = &patterns["full-name"] else {
        panic!("every rule is a context");
    };
    let Pattern::Sequence(sequence) = context.child() else {
        panic!("a `+` builds a sequence");
    };
    let names: Vec<&str> = sequence.children().iter().map(Pattern::name).collect();
    assert_eq!(
        names,
        [
            "not-jack",
            "first-name",
            "space",
            "optional-middle-name-with-space",
            "last-name"
        ]
    );
}

// ─── Repeats ────────────────────────────────────────────────────────────

fn repeat_options(min: usize, max: Option<usize>) -> RepeatOptions {
    RepeatOptions {
        min,
        max,
        ..RepeatOptions::default()
    }
}

/// The repeat inside the rule `rule`'s context.
fn rule_repeat<'p>(patterns: &'p Patterns, rule: &str) -> &'p Repeat {
    let Pattern::Context(context) = &patterns[rule] else {
        panic!("every rule is a context");
    };
    match context.child() {
        Pattern::Repeat(repeat) => repeat,
        Pattern::Optional(optional) => match optional.child() {
            Pattern::Repeat(repeat) => repeat,
            other => panic!("not a repeat: {other:?}"),
        },
        other => panic!("not a repeat: {other:?}"),
    }
}

#[test]
fn repeat() {
    let patterns = build(
        r"
            digit = /\d/
            digits = (digit)+
        ",
    );
    let digit = re("digit", r"\d");
    let digits = rep("digits", digit.clone(), RepeatOptions::default());
    assert!(patterns["digits"].is_equal(&wrapped("digits", digits, vec![digit])));
}

#[test]
fn repeat_zero_or_more() {
    let patterns = build(
        r"
            digit = /\d/
            digits = (digit)*
        ",
    );
    let digit = re("digit", r"\d");
    let digits = opt(
        "optional-digits",
        rep("digits", digit.clone(), repeat_options(0, None)),
    );
    assert!(patterns["digits"].is_equal(&wrapped("optional-digits", digits, vec![digit])));

    // v12 names the optional after the rule and the repeat inside it
    // `inner-optional-<rule>`.
    let Pattern::Context(context) = &patterns["digits"] else {
        panic!("every rule is a context");
    };
    assert_eq!(context.child().name(), "digits");
    assert_eq!(
        rule_repeat(&patterns, "digits").child().name(),
        "digit",
        "the repeated pattern"
    );
    let Pattern::Optional(optional) = context.child() else {
        panic!("`*` builds an optional");
    };
    assert_eq!(optional.child().name(), "inner-optional-digits");
}

#[test]
fn repeat_lower_limit() {
    let patterns = build(
        r"
            digit = /\d+/
            digits = (digit){1,}
        ",
    );
    let digit = re("digit", r"\d+");
    let digits = rep("digits", digit.clone(), repeat_options(1, None));
    assert!(patterns["digits"].is_equal(&wrapped("digits", digits, vec![digit])));
}

#[test]
fn repeat_bounded() {
    let patterns = build(
        r"
            digit = /\d+/
            digits = (digit){1,3}
        ",
    );
    let digit = re("digit", r"\d+");
    let digits = rep("digits", digit.clone(), repeat_options(1, Some(3)));
    assert!(patterns["digits"].is_equal(&wrapped("digits", digits, vec![digit])));
}

#[test]
fn repeat_upper_limit() {
    let patterns = build(
        r"
            digit = /\d+/
            digits = (digit){,3}
        ",
    );
    let digit = re("digit", r"\d+");
    let digits = rep("digits", digit.clone(), repeat_options(0, Some(3)));
    assert!(patterns["digits"].is_equal(&wrapped("digits", digits, vec![digit])));
    assert_eq!(rule_repeat(&patterns, "digits").max(), Some(3));
}

#[test]
fn repeat_exact() {
    let patterns = build(
        r"
            digit = /\d+/
            digits = (digit){3}
        ",
    );
    let digit = re("digit", r"\d+");
    let digits = rep("digits", digit.clone(), repeat_options(3, Some(3)));
    assert!(patterns["digits"].is_equal(&wrapped("digits", digits, vec![digit])));
    let repeat = rule_repeat(&patterns, "digits");
    assert_eq!((repeat.min(), repeat.max()), (3, Some(3)));
}

#[test]
fn repeat_divider() {
    let patterns = build(
        r#"
            digit = /\d+/
            comma = ","
            digits = (digit, comma){3}
        "#,
    );
    let digit = re("digit", r"\d+");
    let comma = lit("comma", ",");
    let digits = rep(
        "digits",
        digit.clone(),
        RepeatOptions {
            divider: Some(comma.clone()),
            ..repeat_options(3, Some(3))
        },
    );
    assert!(patterns["digits"].is_equal(&wrapped("digits", digits, vec![digit, comma])));
}

#[test]
fn repeat_divider_with_trim_divider() {
    let patterns = build(
        r#"
            digit = /\d+/
            comma = ","
            digits = (digit, comma trim)+
        "#,
    );
    let digit = re("digit", r"\d+");
    let comma = lit("comma", ",");
    let digits = rep(
        "digits",
        digit.clone(),
        RepeatOptions {
            divider: Some(comma.clone()),
            trim_divider: true,
            ..RepeatOptions::default()
        },
    );
    assert!(patterns["digits"].is_equal(&wrapped("digits", digits, vec![digit, comma])));
    assert!(rule_repeat(&patterns, "digits").trim_divider());
}

#[test]
fn repeat_divider_with_trim_divider_and_bounds() {
    let patterns = build(
        r#"
            digit = /\d+/
            comma = ","
            digits = (digit, comma trim){3, 3}
        "#,
    );
    let digit = re("digit", r"\d+");
    let divider = lit("comma", ",");
    let digits = rep(
        "digits",
        digit.clone(),
        RepeatOptions {
            divider: Some(divider.clone()),
            trim_divider: true,
            ..repeat_options(3, Some(3))
        },
    );
    assert!(patterns["digits"].is_equal(&wrapped("digits", digits, vec![digit, divider])));
}

#[test]
fn deviation_a_repeat_of_a_repeat_keeps_its_own_divider_and_count() {
    // v12 searches the whole repeat for its divider and quantifier, so the
    // outer repeat here took the inner one's `,` and `+`, and "a,a;a"
    // didn't match. Each repeat's parts are its own here (DESIGN §18).
    let mut patterns = build(r#"x = (("a", ",")+, ";"){2}"#);
    assert_eq!(value(&mut patterns, "x", "a,a;a").as_deref(), Some("a,a;a"));
    assert_eq!(value(&mut patterns, "x", "a,a"), None);

    let mut patterns = build(r#"x = (("a")*)+"#);
    let repeat = rule_repeat(&patterns, "x");
    assert_eq!((repeat.min(), repeat.max()), (1, None));
    assert_eq!(value(&mut patterns, "x", "aa").as_deref(), Some("aa"));
}

// ─── References and aliases ─────────────────────────────────────────────

#[test]
fn reference_through_a_choice_and_a_repeat() {
    let mut patterns = build(
        r#"
            digit = /\d+/
            divider = /\s*,\s*/
            open-bracket = "["
            close-bracket = "]"
            spaces = /\s+/
            items = digit | array
            array-items = (items, divider trim)*
            array = open-bracket + spaces? + array-items + spaces? + close-bracket
        "#,
    );
    let result = patterns["array"].exec_recording("[1, []]");
    assert_eq!(result.tree().unwrap().root().value(), "[1, []]");
}

#[test]
fn alias() {
    let patterns = build(
        r"
            name = /regex/
            alias = name
        ",
    );
    let expected_name = re("name", "regex");
    let expected_alias = re("alias", "regex");
    let contextual_alias = wrapped("alias", expected_alias.clone(), vec![expected_name.clone()]);
    let contextual_name = wrapped("name", expected_name, vec![expected_alias]);

    assert!(patterns["name"].is_equal(&contextual_name));
    assert!(patterns["alias"].is_equal(&contextual_alias));
    // An alias is the pattern it names, renamed.
    let Pattern::Context(alias) = &patterns["alias"] else {
        panic!("every rule is a context");
    };
    assert_eq!(alias.child().name(), "alias");
}

#[test]
fn bad_grammar_at_beginning() {
    assert_eq!(
        message("//"),
        "[Invalid Grammar] Error at line 1, column 1. Hint: syntax, use params, import, @, @"
    );
}

#[test]
fn bad_grammar_further_in() {
    assert_eq!(
        message(
            "name = /\\w/
                age = /()
            "
        ),
        "[Invalid Grammar] Error at line 2, column 23. Hint: !,?,(,(,["
    );
}

#[test]
fn anonymous_patterns() {
    let patterns = build(
        r#"
            complex-expression = !"NOT_THIS" + "Text"? + /regex/ + ("Text" | /regex/ | (pattern)+) + (pattern | pattern)
            pattern = "p"
        "#,
    );
    let expected = wrapped(
        "complex-expression",
        seq(
            "complex-expression",
            vec![
                not("not-NOT_THIS", lit("NOT_THIS", "NOT_THIS")),
                opt("Text", lit("Text", "Text")),
                re("regex", "regex"),
                choice(
                    "anonymous",
                    vec![
                        lit("Text", "Text"),
                        re("regex", "regex"),
                        rep("anonymous", reference("pattern"), RepeatOptions::default()),
                    ],
                ),
                choice(
                    "anonymous",
                    vec![reference("pattern"), reference("pattern")],
                ),
            ],
        ),
        vec![],
    );
    assert!(patterns["complex-expression"].is_equal(&expected));
}

#[test]
fn grammar_with_spaces() {
    let patterns = build(
        "
        john = \"John\"
            \n        jane = \"Jane\"
        ",
    );
    assert!(patterns.contains("john"));
    assert!(patterns.contains("jane"));
}

#[test]
fn expression_pattern() {
    let mut patterns = build(
        r#"
            variables = "a" | "b" | "c"
            ternary = expression + " ? " + expression + " : " + expression
            expression = ternary | variables

            bad-ternary = bad-expression + " ? " + bad-expression + " : " + bad-expression
            bad-expression = bad-ternary | bad-ternary
        "#,
    );
    let result = patterns["expression"].exec("a ? b : c");
    assert!(result.error.is_none());
    let root = result.tree().unwrap().root();
    assert_eq!(root.value(), "a ? b : c");
    assert_eq!(root.name(), "ternary");
    let values: Vec<&str> = root.children().map(|child| child.value()).collect();
    assert_eq!(values, ["a", " ? ", "b", " : ", "c"]);
}

#[test]
fn expression_pattern_with_right_association() {
    let mut patterns = build(
        r#"
            variables = "a" | "b" | "c" | "d" | "e"
            ternary = expression + " ? " + expression + " : " + expression
            expression = ternary right | variables
        "#,
    );
    let result = patterns["expression"].exec("a ? b : c ? d : e");
    assert!(result.error.is_none());
    let root = result.tree().unwrap().root();
    assert_eq!(root.value(), "a ? b : c ? d : e");

    // Right associative: the nested ternary is the LAST operand, so this
    // reads as "a ? b : (c ? d : e)".
    assert_eq!(root.name(), "ternary");
    assert_eq!(root.children().next().unwrap().value(), "a");
    let nested = root.children().nth(4).unwrap();
    assert_eq!(nested.name(), "ternary");
    assert_eq!(nested.value(), "c ? d : e");
}

#[test]
fn take_until() {
    let patterns = build(
        r#"
            script-text = ?->| "</script"
        "#,
    );
    let mut script_text = patterns["script-text"].clone();
    let mut cursor = Cursor::new("function(){}</script");
    let node = script_text.parse(&mut cursor).expect("takes the script");
    assert_eq!((cursor.node_start(node), cursor.node_end(node)), (0, 12));
}

// ─── Unknown pattern names ──────────────────────────────────────────────

#[test]
fn a_reference_to_a_name_defined_nowhere_names_the_name_and_the_rule() {
    assert_eq!(
        message(r#"x = missing + "b""#),
        "[Invalid Grammar] unknown pattern `missing` used in `x`; define it in this grammar, \
         import it, or pass it as a param."
    );
    assert_eq!(
        build_error(r#"x = missing + "b""#),
        GrammarError::UnknownPattern {
            name: "missing".into(),
            used_in: "x".into()
        }
    );
}

#[test]
fn nested_references_are_checked_too() {
    assert_eq!(
        build_error(
            r#"
            a = "a"
            x = a + ("b" | (nope)+)
        "#
        ),
        GrammarError::UnknownPattern {
            name: "nope".into(),
            used_in: "x".into()
        }
    );
}

#[test]
fn a_forward_reference_to_a_pattern_defined_later_is_fine() {
    let mut patterns = build(
        r#"
            x = later + "!"
            later = "a"
        "#,
    );
    assert_eq!(value(&mut patterns, "x", "a!").as_deref(), Some("a!"));
}

#[test]
fn an_alias_of_an_unknown_name_is_an_error() {
    assert_eq!(
        build_error("x = missing"),
        GrammarError::UnknownPattern {
            name: "missing".into(),
            used_in: "x".into()
        }
    );
}

#[test]
fn exporting_an_unknown_name_is_an_error() {
    assert_eq!(
        message("missing"),
        "[Invalid Grammar] unknown pattern `missing` used in `export missing`; define it in \
         this grammar, import it, or pass it as a param."
    );
}

#[test]
fn a_hand_built_reference_is_checked_when_it_runs() {
    // v12 throws "Couldn't find 'not-there' pattern within tree." at first
    // parse; here `exec` reports the name up front, as data (DESIGN §11).
    let mut reference = reference("not-there");
    let result = reference.exec("x");
    assert!(matches!(
        result.pattern_error,
        Some(PatternError::UnknownReference { .. })
    ));
}

// ─── Shadowed literals ──────────────────────────────────────────────────

#[test]
fn a_literal_that_a_shorter_earlier_literal_shadows() {
    assert_eq!(
        message(r#"x = "=" | "==""#),
        r#"[Invalid Grammar] `"=="` can never match after `"="` in `x`; put the longer literal first."#
    );
}

#[test]
fn the_longer_literal_first_is_fine() {
    let mut patterns = build(r#"x = "==" | "=""#);
    assert_eq!(value(&mut patterns, "x", "==").as_deref(), Some("=="));
    assert_eq!(value(&mut patterns, "x", "=").as_deref(), Some("="));
}

#[test]
fn not_adjacent_and_inside_a_nested_choice() {
    assert_eq!(
        message(r#"x = "a" + ("<" | ">" | "<=")"#),
        r#"[Invalid Grammar] `"<="` can never match after `"<"` in `x`; put the longer literal first."#
    );
}

#[test]
fn looks_through_rule_names_including_ones_defined_later() {
    assert_eq!(
        message(
            r#"
            lt = "<"
            op = lt | le
            le = "<="
        "#
        ),
        r#"[Invalid Grammar] `"<="` can never match after `"<"` in `op`; put the longer literal first."#
    );
}

#[test]
fn looks_through_aliases() {
    assert_eq!(
        message(
            r#"
            less = "<"
            lt = less
            op = lt | "<="
        "#
        ),
        r#"[Invalid Grammar] `"<="` can never match after `"<"` in `op`; put the longer literal first."#
    );
}

#[test]
fn literals_that_are_not_prefixes_of_each_other_are_fine() {
    let mut patterns = build(r#"x = "ab" | "ac" | "b""#);
    assert_eq!(value(&mut patterns, "x", "ac").as_deref(), Some("ac"));
}

#[test]
fn a_regex_or_sequence_before_a_longer_literal_is_not_checked() {
    let mut patterns = build(r#"x = /</ | "<=" | "<" + "!""#);
    assert_eq!(value(&mut patterns, "x", "<").as_deref(), Some("<"));
}

#[test]
fn shadowed_literals_are_quoted_as_json() {
    // v12 prints both literals with `JSON.stringify`: escapes and all.
    assert_eq!(
        message(r#"x = "a\"" | "a\"b\n""#),
        r#"[Invalid Grammar] `"a\"b\n"` can never match after `"a\""` in `x`; put the longer literal first."#
    );
    assert_eq!(
        message(r#"x = "\x01" | "\x01b""#),
        r#"[Invalid Grammar] `"\u0001b"` can never match after `"\u0001"` in `x`; put the longer literal first."#
    );
}

// ─── Alias cycles ───────────────────────────────────────────────────────

#[test]
fn two_aliases_naming_each_other() {
    assert_eq!(
        message(
            "
            a = b
            b = a
        "
        ),
        "[Invalid Grammar] alias cycle `a` -> `b` -> `a`: each rule only renames the next, so \
         none of them matches anything."
    );
}

#[test]
fn a_rule_that_aliases_itself() {
    assert_eq!(
        build_error("a = a"),
        GrammarError::AliasCycle {
            cycle: vec!["a".into(), "a".into()]
        }
    );
}

#[test]
fn a_longer_chain_entered_from_a_rule_outside_the_cycle() {
    assert_eq!(
        build_error(
            "
            entry = a
            a = b
            b = c
            c = a
        "
        ),
        GrammarError::AliasCycle {
            cycle: vec!["a".into(), "b".into(), "c".into(), "a".into()]
        }
    );
}

#[test]
fn a_parenthesized_alias_counts() {
    assert_eq!(
        build_error(
            "
            a = (b)
            b = a
        "
        ),
        GrammarError::AliasCycle {
            cycle: vec!["a".into(), "b".into(), "a".into()]
        }
    );
}

#[test]
fn alias_chains_that_end_in_a_real_pattern_are_fine() {
    let mut patterns = build(
        r#"
            a = b
            b = c
            c = "x"
        "#,
    );
    assert_eq!(value(&mut patterns, "a", "x").as_deref(), Some("x"));
    // The forward alias is the pattern its chain ends in, renamed.
    let Pattern::Context(a) = &patterns["a"] else {
        panic!("every rule is a context");
    };
    assert!(matches!(a.child(), Pattern::Literal(_)));
    assert_eq!(a.child().name(), "a");
}

#[test]
fn recursion_through_a_real_pattern_is_not_an_alias_cycle() {
    let mut patterns = build(
        r#"
            a = b
            b = "(" + a + ")" | "x"
        "#,
    );
    assert_eq!(value(&mut patterns, "a", "((x))").as_deref(), Some("((x))"));
}

// ─── Anonymous pattern names ────────────────────────────────────────────

const ANONYMOUS_SOURCE: &str = r#"
            x = ("a" | "b") + ("c" + "d") + ("e" | "f")
        "#;

/// The names of `x`'s patterns and of the nodes it makes from "acde".
fn names_of_x(cpat: &str) -> (Vec<String>, Vec<String>) {
    let mut patterns = build(cpat);
    let result = patterns["x"].exec("acde");
    let nodes = node_names(result.tree().unwrap());
    (pattern_names(&patterns["x"]), nodes)
}

#[test]
fn building_the_same_grammar_twice_yields_identical_names() {
    let first = names_of_x(ANONYMOUS_SOURCE);
    build(r#"other = ("q" + "r") | ("s" + "t")"#);
    let second = names_of_x(ANONYMOUS_SOURCE);

    assert!(first.1.len() > 1);
    assert_eq!(second, first);
}

#[test]
fn names_are_numbered_from_the_start_of_each_build() {
    build(r#"other = ("q" + "r") | ("s" + "t")"#);
    let (patterns, nodes) = names_of_x(ANONYMOUS_SOURCE);

    let anonymous: Vec<&String> = patterns
        .iter()
        .filter(|name| name.starts_with("anonymous-pattern-"))
        .collect();
    assert_eq!(
        anonymous,
        [
            "anonymous-pattern-1",
            "anonymous-pattern-5",
            "anonymous-pattern-9"
        ]
    );
    // v12's node names for the same parse.
    assert_eq!(nodes, ["x", "a", "anonymous-pattern-5", "c", "d", "e"]);
}

// ─── Every `|` is one choice ────────────────────────────────────────────

#[test]
fn a_choice_builds_the_same_pattern_regardless_of_definition_order() {
    let mut before = build(
        r#"
            a = "a"
            b = "b"
            x = a | b
        "#,
    );
    let mut after = build(
        r#"
            a = "a"
            x = a | b
            b = "b"
        "#,
    );

    let child_type = |patterns: &Patterns| match &patterns["x"] {
        Pattern::Context(context) => context.child().type_name(),
        _ => panic!("every rule is a context"),
    };
    assert_eq!(child_type(&before), "expression");
    assert_eq!(child_type(&after), "expression");

    for text in ["a", "b", "c", "ab", ""] {
        let left = before["x"].exec(text);
        let right = after["x"].exec(text);
        assert_eq!(
            right.tree().map(outline_tree),
            left.tree().map(outline_tree),
            "{text:?}"
        );
        assert_eq!(
            right.furthest_error.as_ref().map(|error| error.start),
            left.furthest_error.as_ref().map(|error| error.start),
            "{text:?}"
        );
    }
    let result = after["x"].exec("b");
    let root = result.tree().unwrap().root();
    assert_eq!(root.name(), "b");
    assert_eq!(root.kind(), NodeKind::Literal);
}

/// `name@start..end[children]` for a whole tree.
fn outline_tree(tree: &Tree<'_>) -> String {
    fn outline(node: cpat_core::Node<'_, '_>, out: &mut String) {
        out.push_str(&format!("{}@{}..{}", node.name(), node.start(), node.end()));
        if node.children().len() > 0 {
            out.push('[');
            for (i, child) in node.children().enumerate() {
                if i > 0 {
                    out.push(' ');
                }
                outline(child, out);
            }
            out.push(']');
        }
    }
    let mut out = String::new();
    outline(tree.root(), &mut out);
    out
}

#[test]
fn an_atom_only_choice_returns_the_matched_alternatives_node_unwrapped() {
    let mut patterns = build(
        r#"
            x = "a" | "b"
        "#,
    );
    let result = patterns["x"].exec("b");
    let root = result.tree().unwrap().root();
    assert_eq!(root.kind(), NodeKind::Literal);
    assert_eq!(root.name(), "b");
}

#[test]
fn mutually_recursive_choices_build_and_parse() {
    let mut patterns = build(
        r#"
            a = b | "c"
            b = a | "d"
        "#,
    );
    assert_eq!(value(&mut patterns, "a", "c").as_deref(), Some("c"));
    assert_eq!(value(&mut patterns, "a", "d").as_deref(), Some("d"));
    assert_eq!(value(&mut patterns, "a", "e"), None);
}

// ─── Greedy options (removed in v12) ────────────────────────────────────

const GREEDY_REMOVED: &str = "greedy options (`<|>`) were removed in v12; order longer \
                              alternatives first, or use a regex such as /==|=/";

#[test]
fn greedy_options_are_rejected_with_a_message_that_teaches_the_fix() {
    let message = message(
        r#"
            hi = "hi"
            hiya = "hiya"
            greeting = hi <|> hiya
        "#,
    );
    assert_eq!(
        message,
        format!("[Invalid Grammar] {GREEDY_REMOVED}. Found: `hi <|> hiya` at line 4, column 24.")
    );
}

#[test]
fn greedy_options_error_names_the_offending_expression() {
    assert_eq!(
        build_error(r#"item = "ab" <|> "abc" <|> "a""#),
        GrammarError::RemovedGreedyOptions {
            found: r#""ab" <|> "abc" <|> "a""#.into(),
            line: 1,
            column: 8
        }
    );
}

#[test]
fn greedy_options_nested_in_a_group_or_sequence_are_rejected_too() {
    assert_eq!(
        message(r#"item = "x" + ("ab" <|> "abc")"#),
        format!(
            "[Invalid Grammar] {GREEDY_REMOVED}. Found: `\"ab\" <|> \"abc\"` at line 1, column 15."
        )
    );
    assert!(message(r#"item = ("ab" <|> "abc")+"#).contains(GREEDY_REMOVED));
}

#[test]
fn greedy_options_error_is_not_a_generic_syntax_error() {
    let message = message(r#"item = "ab" <|> "abc""#);
    assert!(message.contains(GREEDY_REMOVED));
    assert!(!message.contains("Hint:"));
    assert!(message.contains("at line 1, column 8"));
}

#[test]
fn greedy_options_column_counts_utf16_units() {
    // v12 measures the column in UTF-16 units: `é` is one, `😀` two.
    assert_eq!(
        build_error("# é😀\nitem = \"a\" <|> \"b\""),
        GrammarError::RemovedGreedyOptions {
            found: r#""a" <|> "b""#.into(),
            line: 2,
            column: 8
        }
    );
    assert_eq!(
        build_error("x = \"é😀\" + (\"a\" <|> \"b\")"),
        GrammarError::RemovedGreedyOptions {
            found: r#""a" <|> "b""#.into(),
            line: 1,
            column: 14
        }
    );
}

#[test]
fn greedy_bar_inside_a_literal_is_just_text() {
    let mut patterns = build(r#"arrow = "<|>""#);
    assert_eq!(value(&mut patterns, "arrow", "<|>").as_deref(), Some("<|>"));
}

#[test]
fn options_picks_the_first_match_in_order() {
    let mut patterns = build(
        r#"
            hi = "hi"
            hiya = "hiya"
            greeting = hiya | hi
        "#,
    );
    assert_eq!(
        value(&mut patterns, "greeting", "hiya").as_deref(),
        Some("hiya")
    );
    assert_eq!(
        value(&mut patterns, "greeting", "hi").as_deref(),
        Some("hi")
    );
}

// ─── Expression precedence ──────────────────────────────────────────────

#[test]
fn expression_respects_declaration_order_precedence_for_arithmetic() {
    let mut patterns = build(
        r#"
            number = /\d+/
            mul-expr = expr + " * " + expr
            add-expr = expr + " + " + expr
            expr = mul-expr | add-expr | number
        "#,
    );
    let result = patterns["expr"].exec("1 + 2 * 3");
    let root = result.tree().unwrap().root();
    assert_eq!(root.value(), "1 + 2 * 3");
    // `*` binds tighter: the root is the addition.
    assert_eq!(root.name(), "add-expr");
    assert_eq!(root.children().last().unwrap().name(), "mul-expr");
}

#[test]
fn expression_handles_prefix_operator() {
    let mut patterns = build(
        r#"
            number = /\d+/
            neg = "-" + expr
            expr = neg | number
        "#,
    );
    assert_eq!(value(&mut patterns, "expr", "-42").as_deref(), Some("-42"));
}

#[test]
fn expression_handles_postfix_operator() {
    let mut patterns = build(
        r#"
            number = /\d+/
            factorial = expr + "!"
            expr = factorial | number
        "#,
    );
    assert_eq!(value(&mut patterns, "expr", "5!").as_deref(), Some("5!"));
}

#[test]
fn forward_reference_resolves_pattern_defined_later() {
    let mut patterns = build(
        r#"
            greeting = "Hello " + name
            name = /\w+/
        "#,
    );
    assert_eq!(
        value(&mut patterns, "greeting", "Hello World").as_deref(),
        Some("Hello World")
    );
}

#[test]
fn nested_parenthesized_groups_unwrap_correctly() {
    let mut patterns = build(
        r#"
            word = "hello"
            inner = ((word))
        "#,
    );
    assert_eq!(
        value(&mut patterns, "inner", "hello").as_deref(),
        Some("hello")
    );
}

#[test]
fn alias_chain_resolves_through_multiple_levels() {
    let mut patterns = build(
        r#"
            original = "value"
            alias1 = original
            alias2 = alias1
        "#,
    );
    assert_eq!(
        value(&mut patterns, "alias2", "value").as_deref(),
        Some("value")
    );
}

// ─── Edge-case grammars ─────────────────────────────────────────────────

#[test]
fn empty_grammar_throws() {
    assert_eq!(
        message(""),
        "[Invalid Grammar] Error at line 1, column 1. Hint: syntax, use params, import, @, @"
    );
}

#[test]
fn grammar_with_only_comments_produces_no_patterns() {
    assert!(build("# just a comment").is_empty());
}

#[test]
fn grammar_with_syntax_version_and_no_patterns() {
    assert!(build("syntax 1.0").is_empty());
}

#[test]
fn repeat_with_inline_literal_pattern() {
    let mut patterns = build(r#"items = ("item")+"#);
    assert_eq!(
        value(&mut patterns, "items", "itemitemitem").as_deref(),
        Some("itemitemitem")
    );
}

#[test]
fn repeat_with_inline_regex_pattern() {
    let mut patterns = build(r"digits = (/\d/)+");
    assert_eq!(
        value(&mut patterns, "digits", "123").as_deref(),
        Some("123")
    );
}

#[test]
fn inline_literals_use_their_value_as_the_pattern_name() {
    let mut patterns = build(r#"greeting = "Hello" + " " + /\w+/"#);
    let result = patterns["greeting"].exec("Hello World");
    let names: Vec<&str> = result
        .tree()
        .unwrap()
        .root()
        .children()
        .map(|child| child.name())
        .collect();
    assert_eq!(names, ["Hello", " ", r"\w+"]);
}

#[test]
fn five_way_sequence_flattens_correctly() {
    let mut patterns = build(r#"result = "a" + "b" + "c" + "d" + "e""#);
    let result = patterns["result"].exec("abcde");
    let root = result.tree().unwrap().root();
    assert_eq!(root.value(), "abcde");
    // One sequence of five, not nested pairs.
    assert_eq!(root.children().len(), 5);
}

#[test]
fn four_way_options_all_reachable() {
    let mut patterns = build(r#"letter = "a" | "b" | "c" | "d""#);
    assert_eq!(value(&mut patterns, "letter", "a").as_deref(), Some("a"));
    assert_eq!(value(&mut patterns, "letter", "d").as_deref(), Some("d"));
    let Pattern::Context(context) = &patterns["letter"] else {
        panic!("every rule is a context");
    };
    let Pattern::Choice(choice) = context.child() else {
        panic!("a `|` builds a choice");
    };
    assert_eq!(choice.original_alternatives().len(), 4);
}

#[test]
fn sequence_mixing_references_optionals_nots_and_inline_patterns() {
    let mut patterns = build(
        r#"
            keyword = "let"
            ws = /\s+/
            ident = /[a-z]+/
            eq = "="
            digit = /\d+/
            let-stmt = keyword + ws + !digit + ident + ws? + eq + ws? + digit
        "#,
    );
    assert_eq!(
        value(&mut patterns, "let-stmt", "let x=42").as_deref(),
        Some("let x=42")
    );
    assert_eq!(value(&mut patterns, "let-stmt", "let 9=42"), None);
}

#[test]
fn recursive_list_with_nested_expressions() {
    let mut patterns = build(
        r#"
            digit = /\d+/
            comma = ", "
            open = "("
            close = ")"
            item = digit | list
            items = (item, comma trim)+
            list = open + items + close
        "#,
    );
    assert_eq!(
        value(&mut patterns, "list", "(1, (2, 3), 4)").as_deref(),
        Some("(1, (2, 3), 4)")
    );
}

// ─── AST structure ──────────────────────────────────────────────────────

#[test]
fn sequence_children_reflect_pattern_names_in_ast() {
    let mut patterns = build(
        r#"
            first = /[A-Z][a-z]+/
            space = " "
            last = /[A-Z][a-z]+/
            full-name = first + space + last
        "#,
    );
    let result = patterns["full-name"].exec("John Doe");
    let children: Vec<(&str, &str)> = result
        .tree()
        .unwrap()
        .root()
        .children()
        .map(|child| (child.name(), child.value()))
        .collect();
    assert_eq!(
        children,
        [("first", "John"), ("space", " "), ("last", "Doe")]
    );
}

#[test]
fn repeat_ast_contains_repeated_children() {
    let mut patterns = build(
        r"
            digit = /\d/
            digits = (digit)+
        ",
    );
    let result = patterns["digits"].exec("123");
    let values: Vec<&str> = result
        .tree()
        .unwrap()
        .root()
        .children()
        .map(|child| child.value())
        .collect();
    assert_eq!(values, ["1", "2", "3"]);
}

#[test]
fn options_ast_returns_winning_alternative_directly() {
    let mut patterns = build(
        r#"
            a = "aaa"
            b = "bbb"
            choice = a | b
        "#,
    );
    let result = patterns["choice"].exec("bbb");
    let root = result.tree().unwrap().root();
    assert_eq!((root.name(), root.value()), ("b", "bbb"));
}

#[test]
fn expression_ast_wraps_in_expression_node() {
    let mut patterns = build(
        r#"
            num = /\d+/
            add = expr + " + " + expr
            expr = add | num
        "#,
    );
    let result = patterns["expr"].exec("1 + 2");
    assert_eq!(result.tree().unwrap().root().kind(), NodeKind::Expression);
}

#[test]
fn repeat_with_inline_sequence_as_repeated_item() {
    let mut patterns = build(
        r#"
            pairs = ("(" + /\w+/ + ")")+
        "#,
    );
    assert_eq!(
        value(&mut patterns, "pairs", "(a)(b)(c)").as_deref(),
        Some("(a)(b)(c)")
    );
}

#[test]
fn repeat_with_inline_options_as_repeated_item() {
    let mut patterns = build(
        r#"
            bits = ("0" | "1")+
        "#,
    );
    assert_eq!(
        value(&mut patterns, "bits", "1010").as_deref(),
        Some("1010")
    );
}

#[test]
fn take_until_with_regex_terminator() {
    let patterns = build(
        r"
            content = ?->| /[<]/
        ",
    );
    let mut content = patterns["content"].clone();
    let mut cursor = Cursor::new("hello world<");
    let node = content.parse(&mut cursor).expect("takes the text");
    assert_eq!((cursor.node_start(node), cursor.node_end(node)), (0, 11));
}

#[test]
fn inline_not_with_literal_in_sequence() {
    let mut patterns = build(
        r#"
            word = /\w+/
            not-hello = !"hello" + word
        "#,
    );
    assert_eq!(
        value(&mut patterns, "not-hello", "world").as_deref(),
        Some("world")
    );
    assert_eq!(value(&mut patterns, "not-hello", "hello"), None);
}

#[test]
fn inline_optional_with_literal_in_sequence() {
    let mut patterns = build(
        r#"
            greeting = "hello" + " world"?
        "#,
    );
    assert_eq!(
        value(&mut patterns, "greeting", "hello world").as_deref(),
        Some("hello world")
    );
    assert_eq!(
        value(&mut patterns, "greeting", "hello").as_deref(),
        Some("hello")
    );
}

#[test]
fn syntax_version_followed_by_patterns_parses_correctly() {
    let mut patterns = build(
        r#"
            syntax 1.0
            name = "hello"
        "#,
    );
    assert_eq!(
        value(&mut patterns, "name", "hello").as_deref(),
        Some("hello")
    );
}

#[test]
fn empty_string_literal_throws() {
    // v12 throws its `Literal`'s own message, "Value Cannot be empty.";
    // here the literal's construction error is passed on as it is.
    assert_eq!(
        build_error(r#"empty = """#),
        GrammarError::Pattern(PatternError::EmptyLiteral {
            name: "empty".into()
        })
    );
}

#[test]
fn a_regex_that_does_not_build_is_an_error() {
    assert!(matches!(
        build_error("x = /^a/"),
        GrammarError::Pattern(PatternError::AnchoredRegex { .. })
    ));
    assert!(matches!(
        build_error("x = /(/"),
        GrammarError::Pattern(PatternError::InvalidRegex { .. })
    ));
}

// ─── Blocks ─────────────────────────────────────────────────────────────

#[test]
fn block_with_wildcard_content() {
    let mut patterns = build(
        r#"
            braces = ["{"] ... ["}"]
        "#,
    );
    let result = patterns["braces"].exec("{}");
    assert_eq!(result.tree().unwrap().root().kind(), NodeKind::Block);
}

#[test]
fn block_without_content_requires_the_wildcard() {
    assert_eq!(
        message(
            r#"
            braces = ["{"] ["}"]
        "#
        ),
        "[Invalid Grammar] Error at line 3, column 8. Hint: ...,!,?,(,("
    );
}

#[test]
fn block_with_content_pattern() {
    let mut patterns = build(
        r#"
            content = /[^{}]+/
            braces = ["{"] content ["}"]
        "#,
    );
    assert_eq!(
        value(&mut patterns, "braces", "{hello}").as_deref(),
        Some("{hello}")
    );
}

#[test]
fn block_with_multiple_content_patterns() {
    let mut patterns = build(
        r#"
            key = /[a-z]+/
            value = /[a-z]+/
            pair = ["{"] key + ":" + value ["}"]
        "#,
    );
    assert_eq!(
        value(&mut patterns, "pair", "{foo:bar}").as_deref(),
        Some("{foo:bar}")
    );
}

#[test]
fn block_handles_nesting() {
    let patterns = build(
        r#"
            braces = ["{"] ... ["}"]
        "#,
    );
    let block = patterns["braces"]
        .find(|pattern| pattern.type_name() == "block")
        .expect("the rule is a block");
    let mut block = block.pattern.clone();
    let mut cursor = Cursor::new("{ { } }");
    let node = block.parse(&mut cursor).expect("the outer block matches");
    assert_eq!((cursor.node_start(node), cursor.node_end(node)), (0, 7));
}

#[test]
fn block_with_inline_regex_content_in_parens() {
    let mut patterns = build(
        r#"
            parens = ["("] /[^()]+/ [")"]
        "#,
    );
    assert_eq!(
        value(&mut patterns, "parens", "(hello)").as_deref(),
        Some("(hello)")
    );
}

#[test]
fn block_wildcard_matches_any_content() {
    let mut patterns = build(
        r#"
            braces = ["{"] ... ["}"]
        "#,
    );
    assert_eq!(
        value(&mut patterns, "braces", "{anything goes here}").as_deref(),
        Some("{anything goes here}")
    );
}

#[test]
fn block_with_inline_regex_content() {
    let mut patterns = build(
        r#"
            braces = ["{"] /[^{}]*/ ["}"]
        "#,
    );
    assert_eq!(
        value(&mut patterns, "braces", "{hello world}").as_deref(),
        Some("{hello world}")
    );
}

#[test]
fn block_with_inline_literal_content() {
    let mut patterns = build(
        r#"
            parens = ["("] "hello" [")"]
        "#,
    );
    assert_eq!(
        value(&mut patterns, "parens", "(hello)").as_deref(),
        Some("(hello)")
    );
}

#[test]
fn block_with_pattern_reference_content() {
    let mut patterns = build(
        r#"
            word = /[a-z]+/
            braces = ["{"] word ["}"]
        "#,
    );
    assert_eq!(
        value(&mut patterns, "braces", "{hello}").as_deref(),
        Some("{hello}")
    );
}

#[test]
fn block_with_repeat_content() {
    let mut patterns = build(
        r#"
            item = /[a-z]+/
            braces = ["{"] (item, ",")+ ["}"]
        "#,
    );
    assert_eq!(
        value(&mut patterns, "braces", "{a,b,c}").as_deref(),
        Some("{a,b,c}")
    );
}

#[test]
fn block_with_grouped_options_content() {
    let mut patterns = build(
        r#"
            braces = ["{"] ("yes" | "no") ["}"]
        "#,
    );
    assert_eq!(
        value(&mut patterns, "braces", "{yes}").as_deref(),
        Some("{yes}")
    );
    assert_eq!(
        value(&mut patterns, "braces", "{no}").as_deref(),
        Some("{no}")
    );
}

#[test]
fn block_with_inline_sequence_content() {
    let mut patterns = build(
        r#"
            key = /[a-z]+/
            value = /[0-9]+/
            braces = ["{"] key + "=" + value ["}"]
        "#,
    );
    assert_eq!(
        value(&mut patterns, "braces", "{x=42}").as_deref(),
        Some("{x=42}")
    );
}

#[test]
fn block_with_three_part_inline_sequence_content() {
    let mut patterns = build(
        r#"
            a = "a"
            b = "b"
            c = "c"
            braces = ["{"] a + b + c ["}"]
        "#,
    );
    assert_eq!(
        value(&mut patterns, "braces", "{abc}").as_deref(),
        Some("{abc}")
    );
}

#[test]
fn block_with_ungrouped_options_content() {
    let mut patterns = build(
        r#"
            braces = ["{"] "yes" | "no" ["}"]
        "#,
    );
    assert_eq!(
        value(&mut patterns, "braces", "{yes}").as_deref(),
        Some("{yes}")
    );
    assert_eq!(
        value(&mut patterns, "braces", "{no}").as_deref(),
        Some("{no}")
    );
}

#[test]
fn block_delimiters_are_named_open_and_close() {
    let mut patterns = build(r#"braces = ["{"] /[a-z]+/ ["}"]"#);
    let result = patterns["braces"].exec("{ab}");
    let names: Vec<&str> = result
        .tree()
        .unwrap()
        .root()
        .children()
        .map(|child| child.name())
        .collect();
    assert_eq!(names, ["open", "[a-z]+", "close"]);
}

// ─── Statements this milestone doesn't build ────────────────────────────

#[test]
fn import_without_resolver_throws() {
    assert_eq!(
        message(
            r#"
            import { value } from "file.cpat"
            name = value
        "#
        ),
        "No import resolver supplied."
    );
}

#[test]
fn params_and_decorators_are_refused_until_they_are_supported() {
    assert_eq!(
        build_error("use params { value }\nx = \"a\""),
        GrammarError::Unsupported {
            feature: "`use params` statements"
        }
    );
    for decorated in [
        "@tokens([\" \"])\nspaces = /\\s+/",
        "@record\nspaces = /\\s+/",
    ] {
        assert_eq!(
            build_error(decorated),
            GrammarError::Unsupported {
                feature: "decorators"
            }
        );
    }
}

// ─── v12 grammar/patterns.test.ts ───────────────────────────────────────

#[test]
fn patterns_baseline() {
    let mut patterns = build(
        r#"
        first-name = "John"
        last-name = "Doe"
        space = /\s+/
        full-name = first-name + space + last-name
        "#,
    );
    assert_eq!(
        value(&mut patterns, "full-name", "John Doe").as_deref(),
        Some("John Doe")
    );
}

#[test]
fn patterns_simple_markup() {
    let mut patterns = build(
        r#"
        tag-name = /[a-zA-Z_-]+[a-zA-Z0-9_-]*/
        space = /\s+/
        opening-tag = "<" + tag-name + space? + ">"
        closing-tag = "</" + tag-name + space? + ">"
        child = space? + element + space?
        children = (child)*
        element = opening-tag + children + closing-tag
        body = space? + element + space?
        "#,
    );
    let text = "
        <div>
            <div></div>
            <div></div>
        </div>
        ";
    let result = patterns["body"].exec_recording(text);
    let tree = result.tree().expect("the markup matches");
    // v12 removes every node named like `space` and reads the rest.
    let without_spaces: String = tree
        .root()
        .find_all(|node| node.is_leaf() && !node.name().contains("space"))
        .iter()
        .map(|node| node.value())
        .collect();
    assert_eq!(without_spaces, "<div><div></div><div></div></div>");
}

#[test]
fn patterns_expression_pattern() {
    let mut patterns = build(
        r#"
        integer = /\d+/
        operator = "+" | "-" | "*" | "/"
        unary-operator = "+" | "-"
        postfix-operator = "++" | "--"
        binary-expr = expr + operator + expr
        unary-expr = unary-operator + expr
        postfix-expr = expr + postfix-operator
        expr = postfix-expr | unary-expr | binary-expr | integer
        "#,
    );
    let result = patterns["expr"].exec("-10++");
    let root = result.tree().unwrap().root();
    assert_eq!(root.value(), "-10++");
    assert_eq!(root.name(), "unary-expr");

    let operator = root.children().next().unwrap();
    assert_eq!(
        (operator.kind(), operator.name(), operator.value()),
        (NodeKind::Literal, "-", "-")
    );
    let operand = root.children().nth(1).unwrap();
    assert_eq!(
        (operand.kind(), operand.name(), operand.value()),
        (NodeKind::Expression, "postfix-expr", "10++")
    );
    let integer = operand.children().next().unwrap();
    assert_eq!(
        (integer.kind(), integer.name(), integer.value()),
        (NodeKind::Regex, "integer", "10")
    );
    let increment = operand.children().nth(1).unwrap();
    assert_eq!(
        (increment.kind(), increment.name(), increment.value()),
        (NodeKind::Literal, "++", "++")
    );
}

// ─── v12 Not.test.ts, through grammar files ─────────────────────────────

#[test]
fn a_trailing_lookahead_holds_when_nothing_follows() {
    let mut patterns = build(r#"at-end = "a" + !"b""#);
    assert_eq!(value(&mut patterns, "at-end", "a").as_deref(), Some("a"));
    assert_eq!(value(&mut patterns, "at-end", "ab"), None);
}

#[test]
fn a_trailing_lookahead_whose_child_matches_empty_fails() {
    let patterns = build(r#"empty-not = "a" + !("b"?)"#);
    let mut empty_not = patterns["empty-not"].clone();
    let mut cursor = Cursor::new("a");
    assert!(empty_not.parse(&mut cursor).is_none());
    let error = cursor.error().expect("the lookahead failed");
    assert_eq!(error.pattern.name.as_str(), "not-optional-b");
    assert_eq!(error.start, 1);
}

#[test]
fn a_lookahead_before_a_required_child_still_needs_that_child() {
    let mut patterns = build(r#"mid = "a" + !"b" + "c""#);
    assert!(patterns["mid"].test("ac"));
    assert!(!patterns["mid"].test("a"));
}

#[test]
fn lookaheads_and_cuts_are_walked_in_order_at_the_end_of_the_input() {
    // `!"b"` holds, the cut fires, then `"c"` needs text: committed failure.
    let mut patterns = build(r#"x = "a" + !"b" ~/ "c" | "a""#);
    assert_eq!(value(&mut patterns, "x", "a"), None);

    // `!("b"?)` fails first, so the cut after it is never reached.
    let mut patterns = build(r#"y = "a" + !("b"?) ~/ "c" | "a""#);
    assert_eq!(value(&mut patterns, "y", "a").as_deref(), Some("a"));
}

#[test]
fn a_lookahead_inside_a_repeat() {
    let mut patterns = build(
        r#"
      item = "a" + !"b"
      list = (item)+
    "#,
    );
    assert_eq!(value(&mut patterns, "list", "aaa").as_deref(), Some("aaa"));
}

// ─── v12 Context.test.ts 'With Grammar' ─────────────────────────────────

#[test]
fn context_with_grammar() {
    let mut patterns = build(
        r#"
            names = john + space + jane
            john = "John"
            jane = "Jane"
            space = " "
        "#,
    );
    assert_eq!(
        value(&mut patterns, "names", "John Jane").as_deref(),
        Some("John Jane")
    );
}

// ─── The record ─────────────────────────────────────────────────────────

#[test]
fn every_rule_is_a_context_named_after_it_in_definition_order() {
    let patterns = build(
        r#"
            b = "b"
            a = b + c
            c = "c"
            b
        "#,
    );
    let names: Vec<&str> = patterns.names().collect();
    assert_eq!(names, ["b", "a", "c"]);
    for (name, pattern) in patterns.iter() {
        assert_eq!(pattern.type_name(), "context");
        assert_eq!(pattern.name(), name);
    }
}

#[test]
fn a_rule_taken_out_of_the_record_still_resolves_its_names() {
    let mut patterns = build(
        r#"
            a = b + "!"
            b = "b"
        "#,
    );
    let mut a = patterns.remove("a").expect("a is a rule");
    assert!(!patterns.contains("a"));
    drop(patterns);
    assert!(a.test("b!"));
}

#[test]
fn known_limitation_a_forward_reference_to_a_rule_whose_pattern_has_another_name() {
    // v12 puts each rule in scope under its pattern's own name, and `x =
    // "a"?` builds a pattern named `optional-a`: a reference to `x` from a
    // rule defined earlier finds nothing (v12 throws "Couldn't find 'x'
    // pattern within tree." at first parse; `exec` says so up front here).
    // Defined before its use, the rule is copied in and works.
    let mut forward = build("y = x + \"b\"\nx = \"a\"?");
    assert!(matches!(
        forward["y"].exec("ab").pattern_error,
        Some(PatternError::UnknownReference { .. })
    ));

    let mut backward = build("x = \"a\"?\ny = x + \"b\"");
    assert_eq!(value(&mut backward, "y", "ab").as_deref(), Some("ab"));
}

#[test]
fn a_rule_defined_twice_keeps_its_place_and_its_last_definition() {
    let mut patterns = build("x = \"a\"\ny = \"b\"\nx = \"c\"");
    assert_eq!(patterns.names().collect::<Vec<_>>(), ["x", "y"]);
    assert_eq!(value(&mut patterns, "x", "c").as_deref(), Some("c"));
    assert_eq!(value(&mut patterns, "x", "a"), None);
}
