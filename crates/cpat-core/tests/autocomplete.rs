//! v12's `AutoComplete.test.ts`, all 37 tests, plus autocomplete over
//! recursion, Pratt choices, blocks, take-until, contexts and multi-byte
//! text. Every expected value (option texts, start indices, segments,
//! `is_complete`, the error and `error_at_index`, the tree's value) is
//! what v12 returned on the same pattern and text, so each test asserts
//! more than its v12 original, which often checks only some of them. None
//! of v12's tests here uses a grammar file. Positions are byte offsets;
//! they equal v12's UTF-16 offsets on ASCII text.

mod common;

use std::collections::HashMap;

use common::*;
use cpat_core::{
    AutoComplete, AutoCompleteOptions, Pattern, RepeatOptions, Suggestion, locale_compare,
};

fn auto_complete(pattern: Pattern, options: AutoCompleteOptions) -> AutoComplete {
    AutoComplete::new(pattern, options).expect("every name resolves")
}

fn suggest(pattern: Pattern, text: &str) -> Suggestion<'_> {
    auto_complete(pattern, AutoCompleteOptions::default()).suggest_for(text)
}

fn greedy(names: &[&str]) -> AutoCompleteOptions {
    AutoCompleteOptions {
        greedy_pattern_names: names.iter().map(|name| name.to_string()).collect(),
        ..AutoCompleteOptions::default()
    }
}

fn custom(tokens: &[(&str, &[&str])]) -> HashMap<String, Vec<String>> {
    tokens
        .iter()
        .map(|(name, tokens)| {
            let tokens = tokens.iter().map(|token| token.to_string()).collect();
            (name.to_string(), tokens)
        })
        .collect()
}

/// `complete error=S..L at=I ast=V`, with `-` for what is absent.
fn summary(suggestion: &Suggestion<'_>) -> String {
    let error = suggestion
        .error
        .as_ref()
        .map_or("-".to_string(), |e| format!("{}..{}", e.start, e.last));
    let at = suggestion
        .error_at_index
        .map_or("-".to_string(), |at| at.to_string());
    let ast = suggestion
        .tree()
        .map_or("-".to_string(), |tree| format!("{:?}", tree.root().value()));
    let complete = if suggestion.is_complete {
        "complete"
    } else {
        "incomplete"
    };
    format!("{complete} error={error} at={at} ast={ast}")
}

/// Each option as `text@start[segment:pattern …]`.
fn options(suggestion: &Suggestion<'_>) -> Vec<String> {
    suggestion
        .options
        .iter()
        .map(|option| {
            let segments: Vec<String> = option
                .suggestion_sequence
                .iter()
                .map(|segment| format!("{}:{}", segment.text, segment.pattern.name))
                .collect();
            format!(
                "{}@{}[{}]",
                option.text,
                option.start_index,
                segments.join(" ")
            )
        })
        .collect()
}

fn last_names() -> Pattern {
    choice("last-name", vec![lit("smith", "Smith"), lit("doe", "Doe")])
}

fn john_space_last_name() -> Pattern {
    seq(
        "name",
        vec![lit("john", "John"), lit("space", " "), last_names()],
    )
}

fn first_space_last() -> Pattern {
    seq(
        "full-name",
        vec![
            choice("first-name", vec![lit("jack", "Jack"), lit("john", "John")]),
            lit("space", " "),
            choice("last-name", vec![lit("doe", "Doe"), lit("smith", "Smith")]),
        ],
    )
}

fn kahns() -> Pattern {
    choice(
        "kahns",
        vec![
            lit("large", "kahnnnnnn"),
            lit("medium", "kahnnnnn"),
            lit("small", "kahn"),
        ],
    )
}

fn piped(trim: bool) -> Pattern {
    rep(
        "repeat",
        lit("a", "a"),
        RepeatOptions {
            divider: Some(lit("pipe", "|")),
            trim_divider: trim,
            ..RepeatOptions::default()
        },
    )
}

fn letters_after_separator(start: Option<Pattern>) -> Pattern {
    let mut children: Vec<Pattern> = start.into_iter().collect();
    children.push(lit("separator", "=="));
    children.push(choice(
        "letters-options",
        vec![lit("letter", "A"), lit("letter", "B"), lit("letter", "C")],
    ));
    seq("full-sequence", children)
}

/// v12's `generateExpression(["FlagA", "FlagB", "FlagC"])`.
fn flag_expression() -> Pattern {
    let space = re("[space]", r"\s");
    let boolean_operator = choice(
        "booleanOperator",
        vec![lit("sequence-literal", "AND"), lit("options-literal", "OR")],
    );
    let operator_with_spaces = seq(
        "operator-with-spaces",
        vec![space.clone(), boolean_operator, space],
    );
    let flags = choice(
        "flags",
        ["FlagA", "FlagB", "FlagC"]
            .map(|flag| lit("flag-name", flag))
            .to_vec(),
    );
    let group = seq(
        "group",
        vec![
            lit("open-paren", "("),
            reference("flag-expression"),
            lit("close-paren", ")"),
        ],
    );
    let body = seq(
        "flag-body",
        vec![
            opt("optional-not", lit("not", "NOT ")),
            choice("flag-or-group", vec![flags, group]),
        ],
    );
    rep(
        "flag-expression",
        body,
        RepeatOptions {
            divider: Some(operator_with_spaces),
            trim_divider: true,
            ..RepeatOptions::default()
        },
    )
}

// ─── v12's AutoComplete.test.ts ───────────────────────────────────────────

#[test]
fn no_text() {
    let suggestion = suggest(lit("name", "Name"), "");

    assert_eq!(summary(&suggestion), "incomplete error=0..0 at=0 ast=-");
    assert_eq!(options(&suggestion), ["Name@0[Name:name]"]);
}

#[test]
fn full_pattern_match_simple() {
    let suggestion = suggest(john_space_last_name(), "John Doe");

    assert_eq!(
        summary(&suggestion),
        r#"complete error=- at=- ast="John Doe""#
    );
    assert!(options(&suggestion).is_empty());
}

#[test]
fn more_than_one_option() {
    let suggestion = suggest(john_space_last_name(), "John ");

    assert_eq!(summary(&suggestion), "incomplete error=5..5 at=5 ast=-");
    assert_eq!(
        options(&suggestion),
        ["Doe@5[Doe:doe]", "Smith@5[Smith:smith]"]
    );
}

#[test]
fn option_should_error_at_furthest_match_index() {
    // The literal that ran out of text records `last` at the final
    // character, 7; the option starts after it.
    let suggestion = suggest(john_space_last_name(), "John Smi");

    assert_eq!(summary(&suggestion), "incomplete error=7..7 at=7 ast=-");
    assert_eq!(options(&suggestion), ["th@8[Smith:smith]"]);
}

#[test]
fn root_regex_pattern_suggests_custom_tokens() {
    let options_in = AutoCompleteOptions {
        custom_tokens: custom(&[("free-text", &["luke", "leia skywalker", "luke skywalker"])]),
        ..AutoCompleteOptions::default()
    };
    let suggestion = auto_complete(re("free-text", r"[(\w)\s]+"), options_in).suggest_for("luke");

    // A segment keeps its full token; the option is what is left of it.
    assert_eq!(summary(&suggestion), r#"complete error=- at=- ast="luke""#);
    assert_eq!(
        options(&suggestion),
        [" skywalker@4[luke skywalker:free-text]"]
    );
}

#[test]
fn sequence_regex_pattern_suggests_custom_tokens() {
    let pattern = seq(
        "sequence",
        vec![lit("jedi", "jedi "), re("free-text", r"[(\w)\s]+")],
    );
    let options_in = AutoCompleteOptions {
        custom_tokens: custom(&[("free-text", &["luke", "leia skywalker", "luke skywalker"])]),
        ..AutoCompleteOptions::default()
    };
    let suggestion = auto_complete(pattern, options_in).suggest_for("jedi luke sky");

    assert_eq!(
        summary(&suggestion),
        r#"complete error=- at=- ast="jedi luke sky""#
    );
    assert_eq!(
        options(&suggestion),
        ["walker@13[luke skywalker:free-text]"]
    );
}

#[test]
fn full_pattern_match_with_root_repeat() {
    let mut divider = cpat_core::Regex::new("divider", r"\s+,\s+").unwrap();
    cpat_core::Tokenizable::set_tokens(&mut divider, vec![", ".to_string()]);
    let pattern = rep(
        "last-names",
        john_space_last_name(),
        divided(divider.into()),
    );
    let suggestion = suggest(pattern, "John Doe");

    assert_eq!(
        summary(&suggestion),
        r#"complete error=- at=- ast="John Doe""#
    );
    assert_eq!(options(&suggestion), [", @8[, :divider]"]);
}

#[test]
fn partial_simple() {
    let suggestion = suggest(lit("name", "Name"), "Na");

    assert_eq!(summary(&suggestion), "incomplete error=1..1 at=1 ast=-");
    assert_eq!(options(&suggestion), ["me@2[Name:name]"]);
}

#[test]
fn partial_match_with_bad_characters() {
    // Nothing matched, so the root's suggestions are compared with the
    // text: they part after "N".
    let suggestion = suggest(lit("name", "Name"), "Ni");

    assert_eq!(summary(&suggestion), "incomplete error=1..1 at=1 ast=-");
    assert_eq!(options(&suggestion), ["ame@1[Name:name]"]);
}

#[test]
fn complete() {
    let suggestion = suggest(lit("name", "Name"), "Name");

    assert_eq!(summary(&suggestion), r#"complete error=- at=- ast="Name""#);
    assert!(options(&suggestion).is_empty());
}

#[test]
fn options_auto_complete_on_composing_pattern() {
    // "space" is greedy: it is offered joined with every last name, the
    // custom one for the choice included.
    let options_in = AutoCompleteOptions {
        custom_tokens: custom(&[("last-name", &["Sparrow"])]),
        ..greedy(&["space"])
    };
    let suggestion = auto_complete(first_space_last(), options_in).suggest_for("Jack");

    assert_eq!(summary(&suggestion), "incomplete error=4..4 at=4 ast=-");
    assert_eq!(
        options(&suggestion),
        [
            " Doe@4[ :space Doe:doe]",
            " Smith@4[ :space Smith:smith]",
            " Sparrow@4[ :space Sparrow:last-name]",
        ]
    );
}

#[test]
fn options_auto_complete_on_root_pattern() {
    // Two literals share the name "first-name", so each gets the custom
    // token; with dedupe off, both are kept.
    let jack = lit("first-name", "Jack");
    let john = lit("first-name", "John");
    let (jack_id, john_id) = (jack.id(), john.id());
    let pattern = rep(
        "name-list",
        choice("names", vec![jack, john]),
        RepeatOptions {
            divider: Some(lit("divider", ", ")),
            trim_divider: true,
            ..RepeatOptions::default()
        },
    );
    let options_in = AutoCompleteOptions {
        custom_tokens: custom(&[("first-name", &["James"])]),
        disable_dedupe: true,
        ..AutoCompleteOptions::default()
    };
    let suggestion = auto_complete(pattern, options_in).suggest_for("");

    assert_eq!(summary(&suggestion), "incomplete error=0..0 at=0 ast=-");
    assert_eq!(
        options(&suggestion),
        [
            "Jack@0[Jack:first-name]",
            "James@0[James:first-name]",
            "James@0[James:first-name]",
            "John@0[John:first-name]",
        ]
    );
    let id = |index: usize| suggestion.options[index].suggestion_sequence[0].pattern.id;
    assert_eq!(id(0), Some(jack_id));
    assert_eq!(id(1), Some(jack_id));
    assert_eq!(id(2), Some(john_id));
}

#[test]
fn options_auto_complete_on_leaf_pattern() {
    // A custom token on a leaf comes before its own token.
    let options_in = AutoCompleteOptions {
        custom_tokens: custom(&[("space", &["  "])]),
        ..greedy(&["space"])
    };
    let suggestion = auto_complete(first_space_last(), options_in).suggest_for("Jack");

    assert_eq!(summary(&suggestion), "incomplete error=4..4 at=4 ast=-");
    assert_eq!(
        options(&suggestion),
        [
            "  Doe@4[  :space Doe:doe]",
            "  Smith@4[  :space Smith:smith]",
            " Doe@4[ :space Doe:doe]",
            " Smith@4[ :space Smith:smith]",
        ]
    );
}

#[test]
fn match_on_different_pattern_roots() {
    // The options from matches come first, sorted on their own; the merged
    // list is not sorted again.
    let start = lit("start", "John went to");
    let pattern = choice(
        "both",
        vec![
            seq("first", vec![start.clone(), lit("a", "a bank.")]),
            seq("second", vec![start, lit("the", "the store.")]),
        ],
    );
    let suggestion = suggest(pattern, "John went to a gas station.");

    assert_eq!(summary(&suggestion), "incomplete error=12..12 at=12 ast=-");
    assert_eq!(
        options(&suggestion),
        ["the store.@12[the store.:the]", "a bank.@12[a bank.:a]"]
    );
}

#[test]
fn options_on_errors_because_of_string_ending_with_match() {
    let suggestion = suggest(kahns(), "kahn");

    assert_eq!(summary(&suggestion), r#"complete error=- at=- ast="kahn""#);
    assert_eq!(
        options(&suggestion),
        ["nnnnn@4[kahnnnnnn:large]", "nnnn@4[kahnnnnn:medium]"]
    );
}

#[test]
fn options_on_errors_because_of_string_ending_between_matches() {
    let suggestion = suggest(kahns(), "kahnn");

    assert_eq!(
        summary(&suggestion),
        r#"incomplete error=5..5 at=5 ast="kahn""#
    );
    assert_eq!(
        options(&suggestion),
        ["nnnn@5[kahnnnnnn:large]", "nnn@5[kahnnnnn:medium]"]
    );
}

#[test]
fn options_on_errors_because_of_string_ending_match_middle() {
    let suggestion = suggest(kahns(), "kahnnnnn");

    assert_eq!(
        summary(&suggestion),
        r#"complete error=- at=- ast="kahnnnnn""#
    );
    assert_eq!(options(&suggestion), ["n@8[kahnnnnnn:large]"]);
}

#[test]
fn options_on_errors_because_of_string_ending_on_a_variety_with_match() {
    // "kahnnnnnn" (large) comes before "kahnnnnn" (medium), so the parse
    // stops at "kahn" (small) and the text is not complete.
    let pattern = choice(
        "kahns",
        vec![
            lit("different-3", "kahnnnnnnn3"),
            lit("different-21", "kahnnnnnn21"),
            lit("different-22", "kahnnnnnn22"),
            lit("different-2", "kahnnnnnn2"),
            lit("different", "kahnnnnnn1"),
            lit("large", "kahnnnnnn"),
            lit("small", "kahn"),
            lit("medium", "kahnnnnn"),
        ],
    );
    let suggestion = suggest(pattern, "kahnnnnn");

    assert_eq!(
        summary(&suggestion),
        r#"incomplete error=8..8 at=8 ast="kahn""#
    );
    assert_eq!(
        options(&suggestion),
        [
            "nn3@8[kahnnnnnnn3:different-3]",
            "n21@8[kahnnnnnn21:different-21]",
            "n22@8[kahnnnnnn22:different-22]",
            "n2@8[kahnnnnnn2:different-2]",
            "n1@8[kahnnnnnn1:different]",
            "n@8[kahnnnnnn:large]",
        ]
    );
}

#[test]
fn options_on_errors_because_of_string_ending_on_different_branches_with_match() {
    let pattern = choice(
        "kahns",
        vec![
            seq(
                "kahn-combo-3",
                vec![
                    lit("kah", "kah"),
                    seq("partial", vec![lit("n", "n"), lit("three", "3")]),
                ],
            ),
            seq("kahn-combo", vec![lit("kahn", "kahn"), lit("one", "1")]),
            seq("kahn-combo-2", vec![lit("kahn", "kahn"), lit("two", "2")]),
            lit("small", "kahn"),
        ],
    );
    let suggestion = suggest(pattern, "kahn");

    assert_eq!(summary(&suggestion), r#"complete error=- at=- ast="kahn""#);
    assert_eq!(
        options(&suggestion),
        ["2@4[2:two]", "1@4[1:one]", "3@4[3:three]"]
    );
}

#[test]
fn remove_trailing_divider() {
    let suggestion = suggest(piped(true), "a|a|");

    assert_eq!(
        summary(&suggestion),
        r#"incomplete error=4..4 at=4 ast="a|a""#
    );
    assert_eq!(options(&suggestion), ["a@4[a:a]"]);
}

#[test]
fn remove_options_divider() {
    let names = || choice("names", vec![lit("jedi", "luke")]);
    let divider = || {
        choice(
            "options-divider",
            vec![lit("literal-a", "a"), lit("literal-b", "b")],
        )
    };

    // Control: without trimming, the trailing divider is kept.
    let control = suggest(rep("name-list", names(), divided(divider())), "lukea");
    assert_eq!(summary(&control), r#"complete error=- at=- ast="lukea""#);
    assert_eq!(options(&control), ["luke@5[luke:jedi]"]);

    let trimmed = rep(
        "name-list",
        names(),
        RepeatOptions {
            divider: Some(divider()),
            trim_divider: true,
            ..RepeatOptions::default()
        },
    );
    let suggestion = suggest(trimmed, "lukea");
    assert_eq!(
        summary(&suggestion),
        r#"incomplete error=5..5 at=5 ast="luke""#
    );
    assert_eq!(options(&suggestion), ["luke@5[luke:jedi]"]);
}

#[test]
fn expect_divider() {
    let suggestion = suggest(piped(false), "a|a");

    assert_eq!(summary(&suggestion), r#"complete error=- at=- ast="a|a""#);
    assert_eq!(options(&suggestion), ["|@3[|:pipe]"]);
}

#[test]
fn repeat_with_bad_second_repeat() {
    // Options and a clean cursor: the error runs from the option to the
    // end of the text.
    let suggestion = suggest(piped(true), "a|b");

    assert_eq!(
        summary(&suggestion),
        r#"incomplete error=2..3 at=2 ast="a""#
    );
    assert_eq!(options(&suggestion), ["a@2[a:a]"]);
}

#[test]
fn repeat_with_bad_trailing_content() {
    // No options but a tree: the error runs from the tree's end.
    let suggestion = suggest(flag_expression(), "FlagA AND FlagAlkjhgB");

    assert_eq!(
        summary(&suggestion),
        r#"incomplete error=15..21 at=15 ast="FlagA AND FlagA""#
    );
    assert!(options(&suggestion).is_empty());
}

#[test]
fn ordered_options() {
    let full_name = seq(
        "full-name",
        vec![
            choice("first-name", vec![lit("john", "John"), lit("jane", "Jane")]),
            lit("space", " "),
            choice("last-name", vec![lit("doe", "Doe"), lit("smith", "Smith")]),
        ],
    );
    let pattern = choice(
        "names",
        vec![
            lit("john-stockton", "John Stockton"),
            lit("john-johnson", "John Johnson"),
            full_name,
        ],
    );
    let suggestion = suggest(pattern, "John ");

    assert_eq!(summary(&suggestion), "incomplete error=5..5 at=5 ast=-");
    assert_eq!(
        options(&suggestion),
        [
            "Doe@5[Doe:doe]",
            "Smith@5[Smith:smith]",
            "Stockton@5[John Stockton:john-stockton]",
            "Johnson@5[John Johnson:john-johnson]",
        ]
    );
}

#[test]
fn dedup_suggestions() {
    // Two literals named "space" offering " ": one suggestion.
    let pattern = choice(
        "either-branch",
        vec![
            seq("branch-1", vec![lit("space", " "), lit("A", "A")]),
            seq("branch-2", vec![lit("space", " "), lit("B", "B")]),
        ],
    );
    let suggestion = suggest(pattern, "");

    assert_eq!(summary(&suggestion), "incomplete error=0..0 at=0 ast=-");
    assert_eq!(options(&suggestion), [" @0[ :space]"]);
}

#[test]
fn multiple_complex_branches() {
    let pattern = choice(
        "either-branch",
        vec![
            seq(
                "branch-1",
                vec![
                    lit("space-1-1", " "),
                    lit("space-1-2", " "),
                    choice(
                        "branch-1-options",
                        vec![lit("AA", "AA"), lit("AB", "AB"), lit("BC", "BC")],
                    ),
                ],
            ),
            seq(
                "branch-2",
                vec![
                    lit("space-2-1", " "),
                    lit("space-2-2", " "),
                    choice("branch-2-options", vec![lit("BA", "BA"), lit("BB", "BB")]),
                ],
            ),
        ],
    );
    let suggestion = suggest(pattern, "  B");

    assert_eq!(summary(&suggestion), "incomplete error=2..2 at=2 ast=-");
    assert_eq!(
        options(&suggestion),
        ["A@3[BA:BA]", "B@3[BB:BB]", "C@3[BC:BC]"]
    );
}

#[test]
fn recursion_with_or() {
    let pattern = choice(
        "names",
        vec![reference("names"), lit("john", "John"), lit("jane", "Jane")],
    );
    let suggestion = suggest(pattern, "Jo");

    // A cursor error: the furthest position, the literal's `last`.
    assert_eq!(summary(&suggestion), "incomplete error=1..1 at=1 ast=-");
    assert_eq!(options(&suggestion), ["hn@2[John:john]"]);
}

#[test]
fn recursion_with_and() {
    let last_names = choice("last-names", vec![lit("doe", "Doe"), lit("smith", "Smith")]);
    let full_name = seq(
        "full-name",
        vec![
            choice(
                "first-names",
                vec![lit("john", "John"), lit("jane", "Jane")],
            ),
            lit("space", " "),
            last_names.clone(),
        ],
    );
    let pattern = seq("names", vec![full_name, reference("names"), last_names]);
    let suggestion = auto_complete(pattern, greedy(&["space"])).suggest_for("John");

    assert_eq!(summary(&suggestion), "incomplete error=4..4 at=4 ast=-");
    assert_eq!(
        options(&suggestion),
        [" Doe@4[ :space Doe:doe]", " Smith@4[ :space Smith:smith]"]
    );
}

fn comma() -> Pattern {
    let mut comma = cpat_core::Regex::new("comma", r"\s*,\s*").unwrap();
    cpat_core::Tokenizable::set_tokens(&mut comma, vec![", ".to_string()]);
    comma.into()
}

#[test]
fn repeat_with_options() {
    // The trailing divider is kept (no trimming), so the text is complete
    // and every name is offered after it.
    let names = choice("names", vec![lit("john", "John"), lit("jane", "Jane")]);
    let suggestion = suggest(rep("names-list", names, divided(comma())), "John, ");

    assert_eq!(
        summary(&suggestion),
        r#"complete error=- at=- ast="John, ""#
    );
    assert_eq!(
        options(&suggestion),
        ["Jane@6[Jane:jane]", "John@6[John:john]"]
    );
}

#[test]
fn repeat_with_options_with_options() {
    // "comma" is greedy, so the divider's token is joined with every name
    // that may follow it, the nested choice's too.
    let other_names = choice("names", vec![lit("jack", "Jack"), lit("jill", "Jill")]);
    let names = choice(
        "names",
        vec![lit("john", "John"), lit("jane", "Jane"), other_names],
    );
    let list = rep(
        "names-list",
        names,
        RepeatOptions {
            divider: Some(comma()),
            trim_divider: true,
            ..RepeatOptions::default()
        },
    );
    let suggestion = auto_complete(list, greedy(&["comma"])).suggest_for("John");

    assert_eq!(summary(&suggestion), r#"complete error=- at=- ast="John""#);
    assert_eq!(
        options(&suggestion),
        [
            ", Jack@4[, :comma Jack:jack]",
            ", Jane@4[, :comma Jane:jane]",
            ", Jill@4[, :comma Jill:jill]",
            ", John@4[, :comma John:john]",
        ]
    );
}

#[test]
fn mid_sequence_suggestion() {
    // The greedy optional space offers " " + operand; the operand alone
    // comes from the optional being skippable.
    let optional_space = opt("optional-sapce", re("space", r"\s+"));
    let pattern = seq(
        "statement",
        vec![
            choice(
                "left-operands",
                vec![
                    lit("variable-1", "variable1"),
                    lit("variable-2", "variable2"),
                ],
            ),
            optional_space.clone(),
            choice("operator", vec![lit("==", "=="), lit("!=", "!=")]),
            optional_space,
            choice(
                "right-operands",
                vec![
                    lit("variable-3", "variable3"),
                    lit("variable-4", "variable4"),
                ],
            ),
        ],
    );
    let options_in = AutoCompleteOptions {
        custom_tokens: custom(&[("space", &[" "])]),
        ..greedy(&["space", "operator"])
    };
    let suggestion = auto_complete(pattern, options_in).suggest_for("variable1 ==");

    assert_eq!(summary(&suggestion), "incomplete error=12..12 at=12 ast=-");
    assert_eq!(
        options(&suggestion),
        [
            " variable3@12[ :space variable3:variable-3]",
            " variable4@12[ :space variable4:variable-4]",
            "variable3@12[variable3:variable-3]",
            "variable4@12[variable4:variable-4]",
        ]
    );
}

#[test]
fn suggesting_does_not_consume_the_custom_tokens() {
    // v12 checks its options map is not mutated. Here AutoComplete owns
    // its options, so the claim is that a second call sees them unchanged:
    // the same suggestion, custom token included, joined greedily with the
    // divider.
    let pattern = rep(
        "name-list",
        choice("names", vec![lit("jedi", "luke")]),
        RepeatOptions {
            divider: Some(lit("divider", ", ")),
            trim_divider: true,
            ..RepeatOptions::default()
        },
    );
    let options_in = AutoCompleteOptions {
        custom_tokens: custom(&[("jedi", &["leia"])]),
        ..greedy(&["jedi"])
    };
    let mut auto_complete = auto_complete(pattern, options_in);
    let first = auto_complete.suggest_for("l");
    let second = auto_complete.suggest_for("l");

    let expected = [
        "eia, @1[leia:jedi , :divider]",
        "uke, @1[luke:jedi , :divider]",
    ];
    assert_eq!(summary(&first), "incomplete error=0..0 at=0 ast=-");
    assert_eq!(options(&first), expected);
    assert_eq!(options(&second), expected);
}

#[test]
fn suggests_entire_greedy_node_with_appended_successive_nodes() {
    let pattern = letters_after_separator(Some(lit("start", "start")));
    let suggestion = auto_complete(pattern, greedy(&["separator"])).suggest_for("start");

    assert_eq!(summary(&suggestion), "incomplete error=5..5 at=5 ast=-");
    assert_eq!(
        options(&suggestion),
        [
            "==A@5[==:separator A:letter]",
            "==B@5[==:separator B:letter]",
            "==C@5[==:separator C:letter]",
        ]
    );
}

#[test]
fn incomplete_greedy_text_suggests_node_completion_with_appended_successive_nodes() {
    // The started segment keeps its whole token ("==").
    let pattern = letters_after_separator(Some(lit("start", "start")));
    let suggestion = auto_complete(pattern, greedy(&["separator"])).suggest_for("start=");

    assert_eq!(summary(&suggestion), "incomplete error=5..5 at=5 ast=-");
    assert_eq!(
        options(&suggestion),
        [
            "=A@6[==:separator A:letter]",
            "=B@6[==:separator B:letter]",
            "=C@6[==:separator C:letter]",
        ]
    );
}

#[test]
fn greedy_root_node() {
    let pattern = letters_after_separator(None);
    let suggestion = auto_complete(pattern, greedy(&["separator"])).suggest_for("");

    assert_eq!(summary(&suggestion), "incomplete error=0..0 at=0 ast=-");
    assert_eq!(
        options(&suggestion),
        [
            "==A@0[==:separator A:letter]",
            "==B@0[==:separator B:letter]",
            "==C@0[==:separator C:letter]",
        ]
    );
}

#[test]
fn options_return_farthest_match_with_shared_initial_literals() {
    let bracket = lit("shared-starting-literal", "{");
    let pattern = choice(
        "options",
        vec![
            seq("sequence-one", vec![bracket.clone(), lit("abc", "abc")]),
            seq("sequence-two", vec![bracket, lit("xyz", "xyz")]),
        ],
    );
    let suggestion = suggest(pattern, "{a");

    assert_eq!(summary(&suggestion), "incomplete error=1..1 at=1 ast=-");
    assert_eq!(options(&suggestion), ["bc@2[abc:abc]"]);
}

#[test]
fn dedupe_keys_on_text_and_segment_names() {
    // Same text, different pattern names: both kept, at the root. After a
    // match, the options from matches and errors keep one per text.
    let options_of = || choice("c", vec![lit("x", "a"), lit("y", "a"), lit("x", "a")]);

    let root = suggest(options_of(), "");
    assert_eq!(options(&root), ["a@0[a:x]", "a@0[a:y]"]);

    let after = suggest(seq("s", vec![lit("q", "q"), options_of()]), "q");
    assert_eq!(summary(&after), "incomplete error=1..1 at=1 ast=-");
    assert_eq!(options(&after), ["a@1[a:x]"]);
}

// ─── Beyond v12's tests, checked against v12 ─────────────────────────────

#[test]
fn mutually_recursive_choices_autocomplete_without_overflowing() {
    // a = b | "c";  b = a | "d". The query skips an atom already being
    // expanded further up, so each choice offers the other's literal once.
    let pattern = || {
        choice(
            "a",
            vec![
                choice("b", vec![reference("a"), lit("d", "d")]),
                lit("c", "c"),
            ],
        )
    };

    let empty = suggest(pattern(), "");
    assert_eq!(summary(&empty), "incomplete error=0..0 at=0 ast=-");
    assert_eq!(options(&empty), ["c@0[c:c]", "d@0[d:d]"]);

    let wrong = suggest(pattern(), "x");
    assert_eq!(summary(&wrong), "incomplete error=0..0 at=0 ast=-");
    assert_eq!(options(&wrong), ["c@0[c:c]", "d@0[d:d]"]);

    let extra = suggest(pattern(), "cd");
    assert_eq!(summary(&extra), r#"incomplete error=1..2 at=1 ast="c""#);
    assert!(options(&extra).is_empty());

    // Inside a sequence, what follows the choice is offered after it.
    let in_sequence = seq("s", vec![pattern(), lit("semi", ";")]);
    let after = suggest(in_sequence, "c");
    assert_eq!(summary(&after), "incomplete error=1..1 at=1 ast=-");
    assert_eq!(options(&after), [";@1[;:semi]"]);
}

#[test]
fn after_an_operand_an_operator_or_what_follows_the_expression() {
    // expr = expr "+" expr | /[a-z]/ ;  stmt = expr ";"
    let mut atom = cpat_core::Regex::new("atom", "[a-z]").unwrap();
    cpat_core::Tokenizable::set_tokens(&mut atom, vec!["x".to_string()]);
    let expr = choice(
        "expr",
        vec![
            seq(
                "add",
                vec![reference("expr"), lit("plus", "+"), reference("expr")],
            ),
            atom.into(),
        ],
    );
    let suggestion = suggest(seq("stmt", vec![expr, lit("semi", ";")]), "a");

    assert_eq!(summary(&suggestion), "incomplete error=1..1 at=1 ast=-");
    assert_eq!(options(&suggestion), [";@1[;:semi]", "+@1[+:plus]"]);
}

#[test]
fn blocks_offer_their_content_close_and_what_follows() {
    let mut content = cpat_core::Regex::new("content", "[^{}]+").unwrap();
    cpat_core::Tokenizable::set_tokens(&mut content, vec!["body".to_string()]);
    let statement = || {
        seq(
            "statement",
            vec![
                block("braces", "{", Some(content.clone().into()), "}"),
                lit("semi", ";"),
            ],
        )
    };

    let open = suggest(statement(), "{");
    assert_eq!(summary(&open), "incomplete error=1..1 at=1 ast=-");
    assert_eq!(options(&open), ["body@1[body:content]"]);

    let closed = suggest(statement(), "{x}");
    assert_eq!(summary(&closed), "incomplete error=3..3 at=3 ast=-");
    assert_eq!(options(&closed), [";@3[;:semi]"]);

    let mut digits = cpat_core::Regex::new("digits", "[0-9]+").unwrap();
    cpat_core::Tokenizable::set_tokens(&mut digits, vec!["0".to_string()]);
    let maybe = block("maybe", "{", Some(opt("maybe-digits", digits.into())), "}");
    let optional_content = suggest(maybe, "{");
    assert_eq!(
        summary(&optional_content),
        "incomplete error=1..1 at=1 ast=-"
    );
    assert_eq!(
        options(&optional_content),
        ["}@1[}:close]", "0@1[0:digits]"]
    );

    let wildcard = seq("s", vec![block("b", "<<", None, ">>"), lit("end", "!")]);
    let unclosed = suggest(wildcard, "<<a");
    assert_eq!(summary(&unclosed), "incomplete error=2..2 at=2 ast=-");
    assert_eq!(options(&unclosed), [">>@2[>>:close]"]);
}

#[test]
fn after_a_take_until_comes_what_follows_its_closing_delimiter() {
    // The terminator's probe matched ">" too; v12 gives it no parent, so
    // the sequence's own ">" stays among the leaf matches and offers ".".
    let mut body = cpat_core::TakeUntil::new("body", lit("close", ">"));
    cpat_core::Tokenizable::set_tokens(&mut body, vec!["anything".to_string()]);
    let pattern = || {
        seq(
            "s",
            vec![
                lit("open", "<"),
                body.clone().into(),
                lit("close2", ">"),
                lit("end", "."),
            ],
        )
    };

    let closed = suggest(pattern(), "<abc>");
    assert_eq!(summary(&closed), "incomplete error=5..5 at=5 ast=-");
    assert_eq!(options(&closed), [".@5[.:end]"]);

    let open = suggest(pattern(), "<abc");
    assert_eq!(summary(&open), "incomplete error=4..4 at=4 ast=-");
    assert_eq!(options(&open), [">@4[>:close2]"]);
}

#[test]
fn references_resolve_through_contexts_and_custom_tokens_join_them() {
    let mut value = cpat_core::Regex::new("val", "[0-9]+").unwrap();
    cpat_core::Tokenizable::set_tokens(&mut value, vec!["0".to_string()]);
    let pattern = || {
        context(
            "c",
            seq(
                "s",
                vec![lit("k", "key="), reference("val"), lit("semi", ";")],
            ),
            vec![value.clone().into()],
        )
    };
    let options_in = || AutoCompleteOptions {
        custom_tokens: custom(&[("val", &["true", "42"])]),
        ..AutoCompleteOptions::default()
    };

    let empty_value = auto_complete(pattern(), options_in()).suggest_for("key=");
    assert_eq!(summary(&empty_value), "incomplete error=4..4 at=4 ast=-");
    assert_eq!(
        options(&empty_value),
        ["0@4[0:val]", "42@4[42:val]", "true@4[true:val]"]
    );

    // A match whose pattern still has longer custom tokens offers the rest.
    let started = auto_complete(pattern(), options_in()).suggest_for("key=4");
    assert_eq!(summary(&started), "incomplete error=5..5 at=5 ast=-");
    assert_eq!(options(&started), [";@5[;:semi]", "2@5[42:val]"]);
}

#[test]
fn positions_are_byte_offsets() {
    // v12's UTF-16 offsets 1, 4, 5 and 6 here are bytes 2, 7, 10 and 13.
    let pattern = || {
        seq(
            "s",
            vec![
                lit("e", "é"),
                lit("emoji", "👍"),
                re("sp", r"\s+"),
                choice("end", vec![lit("jp", "日本"), lit("jp2", "日本語")]),
                lit("bang", "!"),
            ],
        )
    };

    let accent = suggest(pattern(), "é");
    assert_eq!(summary(&accent), "incomplete error=2..2 at=2 ast=-");
    assert_eq!(options(&accent), ["👍@2[👍:emoji]"]);

    // The literals that ran out of text recorded their `last` at the start
    // of the last character, which is what selects them.
    let started = suggest(pattern(), "é👍 日");
    assert_eq!(summary(&started), "incomplete error=7..7 at=7 ast=-");
    assert_eq!(options(&started), ["本@10[日本:jp]", "本語@10[日本語:jp2]"]);

    let word = suggest(pattern(), "é👍 日本");
    assert_eq!(summary(&word), "incomplete error=13..13 at=13 ast=-");
    assert_eq!(options(&word), ["!@13[!:bang]"]);
}

#[test]
fn a_greedy_pattern_with_nothing_after_it_offers_nothing() {
    // v12 joins every suggestion with every next one; with no next one,
    // there is nothing to join.
    let pattern = || seq("s", vec![lit("w", "w"), lit("space", " "), lit("w2", "w")]);
    let options_in = || greedy(&["space", "w2"]);

    let empty = auto_complete(pattern(), options_in()).suggest_for("");
    assert_eq!(options(&empty), ["w@0[w:w]"]);
    let after_w = auto_complete(pattern(), options_in()).suggest_for("w");
    assert_eq!(summary(&after_w), "incomplete error=1..1 at=1 ast=-");
    assert!(options(&after_w).is_empty());
}

#[test]
fn greedy_joins_chain_through_the_same_pattern_at_different_places() {
    // `w` and `space` appear twice; each place joins with what follows it,
    // so the chain runs to the non-greedy `x`.
    let w = lit("w", "w");
    let space = lit("space", " ");
    let pattern = seq("s", vec![w.clone(), space.clone(), w, space, lit("x", "x")]);
    let mut auto_complete = auto_complete(pattern, greedy(&["w", "space"]));
    let after_w = auto_complete.suggest_for("w");
    assert_eq!(summary(&after_w), "incomplete error=1..1 at=1 ast=-");
    assert_eq!(options(&after_w), [" w x@1[ :space w:w  :space x:x]"]);

    // Four joins deep.
    let empty = auto_complete.suggest_for("");
    assert_eq!(options(&empty), ["w w x@0[w:w  :space w:w  :space x:x]"]);
}

#[test]
fn only_the_options_that_start_furthest_in_a_list_are_merged() {
    // Nothing matched "a", so the root offers "b" (from the literal "b",
    // parting with the text at 0) and "b" (the rest of "ab", at 1). Each
    // list keeps only its furthest options before the lists merge on text,
    // so "b" at 0 can't stand in for "b" at 1.
    let pattern = choice("c", vec![lit("b", "b"), lit("ab", "ab")]);
    let suggestion = suggest(pattern, "a");

    assert_eq!(summary(&suggestion), "incomplete error=0..0 at=0 ast=-");
    assert_eq!(options(&suggestion), ["b@1[ab:ab]"]);
}

// ─── Deviations from v12 ──────────────────────────────────────────────────

#[test]
fn deviation_a_greedy_join_that_never_ends_offers_nothing() {
    // A greedy child of a repeat is followed by itself, forever: v12
    // recurses until it throws `RangeError`. Here the joins stop after
    // `MAX_GREEDY_DEPTH` and the innermost offers nothing, so the chain
    // does (as a greedy pattern with nothing after it does in v12).
    let repeat = rep("r", lit("a", "a"), RepeatOptions::default());
    let mut auto_complete = auto_complete(repeat, greedy(&["a"]));

    let empty = auto_complete.suggest_for("");
    assert_eq!(summary(&empty), "incomplete error=0..0 at=0 ast=-");
    assert!(options(&empty).is_empty());
    let complete = auto_complete.suggest_for("aa");
    assert_eq!(summary(&complete), r#"complete error=- at=- ast="aa""#);
    assert!(options(&complete).is_empty());

    // Through a reference the places get deeper at every turn.
    let list = seq(
        "list",
        vec![
            lit("item", "i"),
            lit("sep", ","),
            opt("more", reference("list")),
        ],
    );
    let through_reference = auto_complete_with(list, &["item", "sep"]).suggest_for("i");
    assert_eq!(
        summary(&through_reference),
        "incomplete error=1..1 at=1 ast=-"
    );
    assert!(options(&through_reference).is_empty());
}

#[test]
fn deviation_a_branching_greedy_join_stops_at_the_budget() {
    // Each of the two items is greedy and recomputes what follows it, so
    // the expansion doubles at every level: 2^32 places before the depth
    // limit. The budget stops it (a count, not a clock), and the chain,
    // never ending, offers nothing.
    let list = seq(
        "list",
        vec![
            choice("item", vec![lit("i", "a"), lit("i", "b")]),
            opt("more", reference("list")),
        ],
    );
    let suggestion = auto_complete_with(list, &["i"]).suggest_for("a");

    assert_eq!(summary(&suggestion), r#"complete error=- at=- ast="a""#);
    assert!(options(&suggestion).is_empty());
}

fn auto_complete_with(pattern: Pattern, greedy_names: &[&str]) -> AutoComplete {
    auto_complete(pattern, greedy(greedy_names))
}

#[test]
fn an_unknown_name_is_refused_when_auto_complete_is_built() {
    let pattern = seq("s", vec![lit("k", "key="), reference("val")]);

    let error = AutoComplete::new(pattern, AutoCompleteOptions::default()).err();
    assert!(
        matches!(&error, Some(cpat_core::PatternError::UnknownReference { name, .. }) if name == "val"),
        "{error:?}"
    );
}

// ─── Sorting: JS localeCompare ────────────────────────────────────────────

#[test]
fn options_are_sorted_as_js_locale_compare_sorts_them() {
    // The expected order is Node 24's `[...].sort((a, b) => a.localeCompare(b))`
    // on these texts, pasted in: punctuation before digits before letters,
    // ICU's punctuation order, case as a tie-break (lower first), accents
    // before case, a shorter word before a longer one it begins (`á` before
    // `a b`), digits compared one at a time, `æ` as a kind of `ae` and `ß`
    // as a kind of `ss`.
    let words = [
        "Zeta", "alpha", "Alpha", "_x", "-x", "10", "9", "beta", "éclair", "eclair", "Éclair",
        "a b", "a-b", "ab", "aB", "Ab", "AB", "a.b", "a_b", "(x)", "[x]", "{x}", "@x", "*x", "/x",
        "\\x", "&x", "#x", "%x", "`x", "^x", "+x", "<x", "=x", ">x", "|x", "~x", "$x", "!x", "?x",
        ",x", ";x", ":x", "'x", "\"x", " x", "\tx", "x", "X", "a10", "a2", "ä", "à", "á", "â", "ã",
        "å", "æ", "ç", "ø", "ß", "ss", "ñ", "n", "o", "ö", "ő",
    ];
    let node_order = [
        "\tx", " x", "_x", "-x", ",x", ";x", ":x", "!x", "?x", "'x", "\"x", "(x)", "[x]", "{x}",
        "@x", "*x", "/x", "\\x", "&x", "#x", "%x", "`x", "^x", "+x", "<x", "=x", ">x", "|x", "~x",
        "$x", "10", "9", "á", "à", "â", "å", "ä", "ã", "a b", "a_b", "a-b", "a.b", "a10", "a2",
        "ab", "aB", "Ab", "AB", "æ", "alpha", "Alpha", "beta", "ç", "eclair", "éclair", "Éclair",
        "n", "ñ", "o", "ö", "ő", "ø", "ss", "ß", "x", "X", "Zeta",
    ];

    // Reversed, so a missing level can't hide behind a stable sort keeping
    // an order that happened to be right.
    let mut sorted: Vec<&str> = words.iter().rev().copied().collect();
    sorted.sort_by(|a, b| locale_compare(a, b));
    assert_eq!(sorted, node_order);

    // And through AutoComplete, which sorts each list of options.
    let pattern = choice(
        "words",
        words
            .iter()
            .rev()
            .enumerate()
            .map(|(i, word)| lit(&format!("w{i}"), word))
            .collect(),
    );
    let texts: Vec<String> = suggest(pattern, "")
        .options
        .into_iter()
        .map(|option| option.text)
        .collect();
    assert_eq!(texts, node_order);
}

#[test]
fn locale_compare_ignores_control_characters_and_ties_stay_in_order() {
    // JS: "a\u0000".localeCompare("a") === 0, so a stable sort keeps them
    // as they came.
    assert_eq!(locale_compare("a\u{0}", "a"), std::cmp::Ordering::Equal);
    assert_eq!(locale_compare("a\u{ad}b", "ab"), std::cmp::Ordering::Equal);
    assert_eq!(locale_compare("", "a"), std::cmp::Ordering::Less);
    // A decomposed accent weighs as the precomposed one.
    assert_eq!(locale_compare("e\u{301}", "é"), std::cmp::Ordering::Equal);
    // Accents are compared left to right: "eá" before "éa".
    assert_eq!(locale_compare("eá", "éa"), std::cmp::Ordering::Less);
    // A no-break space is a space, told apart only at the case level.
    assert_eq!(locale_compare("a ", "a\u{a0}"), std::cmp::Ordering::Less);
    assert_eq!(locale_compare("A ", "a\u{a0}"), std::cmp::Ordering::Greater);
    // "æ" is "ae" set apart by more than any accent. Node's order:
    let mut digraphs = vec!["æ", "áe", "aé", "AE", "Ae", "ae"];
    digraphs.sort_by(|a, b| locale_compare(a, b));
    assert_eq!(digraphs, ["ae", "Ae", "AE", "aé", "áe", "æ"]);
}

#[test]
fn a_large_choice_offers_every_literal() {
    // Only greedy joins are budgeted: a pattern expands to its leaves once
    // each, so a long keyword list is never cut short.
    let keywords: Vec<String> = (0..20_000).map(|i| format!("k{i:05}")).collect();
    let pattern = choice("keywords", keywords.iter().map(|k| lit("kw", k)).collect());
    let suggestion = suggest(pattern, "k1999");

    assert_eq!(suggestion.options.len(), 10); // k19990 to k19999
    assert_eq!(suggestion.options[0].text, "0");
    assert_eq!(suggestion.options[0].start_index, 5);
    assert_eq!(suggest_count_from_empty(&keywords), 20_000);
}

fn suggest_count_from_empty(keywords: &[String]) -> usize {
    let pattern = choice("keywords", keywords.iter().map(|k| lit("kw", k)).collect());
    suggest(pattern, "").options.len()
}

// ─── Fix round 1: rules no other test pinned, checked against v12 ─────────

#[test]
fn only_errors_ending_at_the_last_character_offer_options() {
    // "pair" fails on "a" too, but its error does not end at the last
    // character, so its custom tokens ("abz" would leave "bz") are not
    // offered: only what follows the matched "a".
    let pattern = seq(
        "s",
        vec![
            seq("pair", vec![lit("a", "a"), lit("b", "b")]),
            lit("c", "c"),
        ],
    );
    let options_in = AutoCompleteOptions {
        custom_tokens: custom(&[("pair", &["ab", "abz", "zz"]), ("c", &["cc"])]),
        ..AutoCompleteOptions::default()
    };
    let mut complete = auto_complete(pattern, options_in);

    let after_a = complete.suggest_for("a");
    assert_eq!(summary(&after_a), "incomplete error=1..1 at=1 ast=-");
    assert_eq!(options(&after_a), ["b@1[b:b]"]);

    let after_ab = complete.suggest_for("ab");
    assert_eq!(summary(&after_ab), "incomplete error=2..2 at=2 ast=-");
    assert_eq!(options(&after_ab), ["c@2[c:c]", "cc@2[cc:c]"]);
}

#[test]
fn an_option_starting_before_the_furthest_is_dropped_whatever_its_order() {
    // "John" parts from "Ja" at 1, after "Jack" and "James" placed at 2
    // came first: it is dropped, not kept beside them.
    let pattern = rep(
        "name-list",
        choice(
            "names",
            vec![lit("first-name", "Jack"), lit("first-name", "John")],
        ),
        RepeatOptions {
            divider: Some(lit("divider", ", ")),
            trim_divider: true,
            ..RepeatOptions::default()
        },
    );
    let options_in = AutoCompleteOptions {
        custom_tokens: custom(&[("first-name", &["James"])]),
        disable_dedupe: true,
        ..AutoCompleteOptions::default()
    };
    let suggestion = auto_complete(pattern, options_in).suggest_for("Ja");

    assert_eq!(summary(&suggestion), "incomplete error=1..1 at=1 ast=-");
    assert_eq!(
        options(&suggestion),
        ["ck@2[Jack:first-name]", "mes@2[James:first-name]"]
    );
}

#[test]
fn a_partial_operator_offers_only_its_completion_and_errors_there() {
    // "OR" would be placed at 6, before "AND" at 7; keeping it would also
    // move the error back to 6.
    let and = suggest(flag_expression(), "FlagA A");
    assert_eq!(summary(&and), r#"incomplete error=7..7 at=7 ast="FlagA""#);
    assert_eq!(options(&and), ["ND@7[AND:sequence-literal]"]);

    let or = suggest(flag_expression(), "FlagA O");
    assert_eq!(summary(&or), r#"incomplete error=7..7 at=7 ast="FlagA""#);
    assert_eq!(options(&or), ["R@7[OR:options-literal]"]);
}
