//! v12's `Expression.test.ts` "Expression: literal-set fast path", on the
//! one `Choice` (its `LiteralSet` mode), plus a differential test against
//! the ordered loop.
//!
//! v12 compares its fast path with a recording parse, which forces the
//! plain loop. Here the plain loop is the same alternatives built with
//! `Choice::ordered`, which never takes the literal-set mode; the recording
//! parse's own comparison is in `tests/history.rs`. v12 counts `Literal.prototype.parse` calls with a jest
//! spy; here the `parse-log` feature logs every `Literal::parse` on the
//! test's own thread, and the tests assert that sequence and its length.
//! Expected trees and errors come from running v12 on the same grammars
//! and inputs (byte offsets; a TS inclusive `lastIndex` L is `last` here,
//! a node's L becomes end L+1).

mod common;

use common::*;
use cpat_core::{
    Choice, ChoiceMode, Cursor, Limits, Match, ParseError, Pattern, RepeatOptions, parse_log,
};

const KEYWORDS: [&str; 11] = [
    "if", "in", "int", "else", "elif", "for", "fo", "while", "x", "é", "😀a",
];

/// The keyword literals, `kw-<token>`.
fn keywords() -> Vec<Pattern> {
    literals(&KEYWORDS)
}

fn literals(tokens: &[&str]) -> Vec<Pattern> {
    tokens.iter().map(|t| lit(&format!("kw-{t}"), t)).collect()
}

/// v12's `keywordChoice`.
fn keyword_choice() -> Pattern {
    choice("keyword", keywords())
}

/// The same alternatives (same ids) in each mode: `(literal set, ordered)`.
fn both(name: &str, alternatives: Vec<Pattern>) -> (Pattern, Pattern) {
    let fast: Pattern = Choice::new(name, alternatives.clone()).unwrap().into();
    let plain: Pattern = Choice::ordered(name, alternatives).unwrap().into();
    assert_eq!(as_choice(&fast).mode(), Some(ChoiceMode::LiteralSet));
    assert_eq!(as_choice(&plain).mode(), Some(ChoiceMode::Ordered));
    (fast, plain)
}

/// Clears the log, so a test sees only its own parses.
fn fresh_log() {
    parse_log::take_literals();
}

// ─── Expression: literal-set fast path ──────────────────────────────────

#[test]
fn only_the_winning_literal_is_parsed() {
    let mut choice = keyword_choice();
    fresh_log();

    let result = choice.exec("while");

    assert_eq!(result.tree().unwrap().root().name(), "kw-while");
    let parsed = parse_log::take_literals();
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed, ["while"]);
}

#[test]
fn no_literal_is_parsed_when_none_can_match() {
    let mut choice = keyword_choice();
    fresh_log();

    assert!(choice.exec("zzz").tree().is_none());
    assert_eq!(parse_log::take_literals().len(), 0);
}

#[test]
fn the_ordered_loop_parses_every_literal_it_tries() {
    // The counterpart of the two tests above: what they count is the
    // index at work, not the parse log missing calls.
    let (_, mut plain) = both("keyword", keywords());
    fresh_log();

    assert!(plain.exec("zzz").tree().is_none());
    assert_eq!(parse_log::take_literals(), KEYWORDS);
}

/// `kw = if-kw | "else"` with `if-kw = "if"`, then `";"`, in a context.
fn referenced_keywords() -> Pattern {
    let kw = choice("kw", vec![reference("if-kw"), lit("else", "else")]);
    let root = seq("root", vec![kw, lit("end", ";")]);
    context("scope", root, vec![lit("if-kw", "if")])
}

fn kw_of(scope: &Pattern) -> &Choice {
    let Pattern::Context(context) = scope else {
        panic!("not a context")
    };
    let Pattern::Sequence(root) = context.child() else {
        panic!("not a sequence")
    };
    as_choice(&root.children()[0])
}

#[test]
fn literals_reached_through_references_count() {
    let mut scope = referenced_keywords();
    // A reference alternative is classified at the first parse, once its
    // name can resolve (DESIGN §8).
    assert_eq!(kw_of(&scope).mode(), None);
    fresh_log();

    let r#else = scope.exec("else;");
    let r#if = scope.exec("if;");

    assert_eq!(r#else.tree().unwrap().root().value(), "else;");
    assert_eq!(r#if.tree().unwrap().root().value(), "if;");
    assert_eq!(kw_of(&scope).mode(), Some(ChoiceMode::LiteralSet));
    // The `if` literal is never tried against "else", nor `else` against
    // "if".
    assert_eq!(parse_log::take_literals(), ["else", ";", "if", ";"]);
}

#[test]
fn a_winner_reached_through_a_reference_is_named_after_it() {
    // The winning alternative is parsed in place, so a reference names the
    // node as it would in the loop.
    let kw = choice(
        "kw",
        vec![reference_to("my-if", "if-kw"), lit("else", "else")],
    );
    let mut scope = context("scope", kw, vec![lit("if-kw", "if")]);

    assert_eq!(
        exec_outline(&mut scope, "if"),
        outlined("my-if@0..2", "-", "-")
    );
    assert_eq!(
        exec_outline(&mut scope, "iz"),
        // `exec` on a context names its child in the summary (DESIGN §11).
        outlined("-", "kw@0..2", "my-if@0..1")
    );
}

/// v12's results for `keywordChoice().exec(text)`: `(text, tree, error,
/// furthest error)`. v12's fast path and plain loop agree on all of them.
const KEYWORD_RESULTS: &[(&str, &str, &str, &str)] = &[
    ("", "-", "-", "-"),
    ("i", "-", "keyword@0..1", "kw-if@0..0"),
    ("if", "kw-if@0..2", "-", "-"),
    ("in", "kw-in@0..2", "-", "kw-if@0..1"),
    ("int", "-", "keyword@2..3", "kw-if@0..1"),
    ("inx", "-", "keyword@2..3", "kw-if@0..1"),
    ("el", "-", "keyword@0..2", "kw-else@0..1"),
    ("els", "-", "keyword@0..3", "kw-else@0..2"),
    ("elif", "kw-elif@0..4", "-", "kw-else@0..2"),
    ("else", "kw-else@0..4", "-", "kw-if@0..0"),
    ("elsewhere", "-", "keyword@4..9", "kw-if@0..0"),
    ("f", "-", "keyword@0..1", "kw-if@0..0"),
    ("fo", "kw-fo@0..2", "-", "kw-for@0..1"),
    ("for", "kw-for@0..3", "-", "kw-if@0..0"),
    ("forx", "-", "keyword@3..4", "kw-if@0..0"),
    ("w", "-", "keyword@0..1", "kw-if@0..0"),
    ("whil", "-", "keyword@0..4", "kw-while@0..3"),
    ("x", "kw-x@0..1", "-", "kw-if@0..0"),
    ("y", "-", "keyword@0..1", "kw-if@0..0"),
    ("é", "kw-é@0..2", "-", "kw-if@0..0"),
    ("😀a", "kw-😀a@0..5", "-", "kw-if@0..0"),
    ("😀b", "-", "keyword@0..5", "kw-😀a@0..4"),
    ("e\u{301}", "-", "keyword@0..3", "kw-else@0..1"),
    ("e\u{300}", "-", "keyword@0..3", "kw-else@0..1"),
];

#[test]
fn results_furthest_error_and_leaf_match_match_the_plain_loop() {
    for &(text, tree, error, furthest) in KEYWORD_RESULTS {
        let (mut fast, mut plain) = both("keyword", keywords());

        let a = observe_exec(&mut fast, text);
        let b = observe_exec(&mut plain, text);

        assert_eq!(a, b, "{text:?}");
        assert_eq!(
            exec_outline(&mut fast, text),
            outlined(tree, error, furthest),
            "{text:?} against v12"
        );
    }
}

/// v12's results for `stmt = keywordChoice() ";"`.
const STATEMENT_RESULTS: &[(&str, &str, &str, &str)] = &[
    ("int;", "-", "stmt@0..4", "semi@2..2"),
    ("inx;", "-", "stmt@0..4", "semi@2..2"),
    ("els;", "-", "stmt@0..4", "kw-else@0..3"),
    ("elif", "-", "stmt@0..4", "stmt@0..4"),
    ("zz;", "-", "stmt@0..3", "kw-if@0..0"),
    ("for;;", "-", "stmt@4..5", "kw-if@0..0"),
    ("é;", "stmt@0..3[kw-é@0..2 semi@2..3]", "-", "kw-if@0..0"),
];

#[test]
fn inside_a_sequence_the_furthest_error_matches_the_plain_loop() {
    for &(text, tree, error, furthest) in STATEMENT_RESULTS {
        let (fast, plain) = both("keyword", keywords());
        let mut fast = seq("stmt", vec![fast, lit("semi", ";")]);
        let mut plain = seq("stmt", vec![plain, lit("semi", ";")]);

        let a = observe_exec(&mut fast, text);
        let b = observe_exec(&mut plain, text);

        assert_eq!(a, b, "{text:?}");
        assert_eq!(
            exec_outline(&mut fast, text),
            outlined(tree, error, furthest),
            "{text:?} against v12"
        );
    }
}

#[test]
fn at_the_end_of_the_text_the_ordered_loop_runs() {
    // v12 replays the loop there: every literal fails without comparing.
    // (A sequence that reaches the end stops before its next child, so the
    // choice is driven directly.)
    let (mut fast, mut plain) = both("keyword", keywords());
    fresh_log();

    let a = observe_parse_at(&mut fast, "<", 1);
    let parsed = parse_log::take_literals();
    let b = observe_parse_at(&mut plain, "<", 1);

    assert_eq!(parsed, KEYWORDS);
    assert_eq!(a, b);
    assert_eq!(a.error, "keyword@1..1");
    assert_eq!(a.furthest, "kw-if@1..1");
}

#[test]
fn deviation_a_literal_sharing_only_a_high_surrogate_fails_at_the_start() {
    // v12 compares UTF-16 units, so `😀a` against `😁` (D83D DE00 against
    // D83D DE01) gets one unit in, to `last` 1, inside the emoji, and beats
    // every literal that failed at 0. `Literal` compares chars here (a
    // position inside a char doesn't exist), so `😀a` fails at 0 like the
    // rest and the first literal tried is the furthest: v12 reports
    // `kw-😀a@0..1` (UTF-16). Both modes agree, as they do in v12.
    let (mut fast, mut plain) = both("keyword", keywords());

    assert_eq!(
        exec_outline(&mut fast, "😁"),
        outlined("-", "keyword@0..4", "kw-if@0..0")
    );
    assert_eq!(
        observe_exec(&mut fast, "😁"),
        observe_exec(&mut plain, "😁")
    );
}

// ─── Which mode a choice takes ──────────────────────────────────────────

fn mode_of(pattern: &Pattern) -> Option<ChoiceMode> {
    as_choice(pattern).mode()
}

#[test]
fn only_literals_take_the_literal_set_mode() {
    assert_eq!(mode_of(&keyword_choice()), Some(ChoiceMode::LiteralSet));

    let not_literals = [
        ("regex", re("digits", "[0-9]+")),
        ("sequence", seq("pair", vec![lit("a", "a"), lit("b", "b")])),
        ("choice", choice("inner", vec![lit("a", "a")])),
        ("optional", opt("maybe", lit("a", "a"))),
        ("marked", right(lit("a", "a"))),
    ];
    for (what, atom) in not_literals {
        let mut alternatives = keywords();
        alternatives.insert(3, atom);
        let pattern = choice("keyword", alternatives);
        assert_eq!(mode_of(&pattern), Some(ChoiceMode::Ordered), "{what}");
    }
}

#[test]
fn a_choice_with_an_operator_is_not_a_literal_set() {
    // `kw = kw "!" | "a" | "b"`: every atom is a literal, but there is a
    // postfix operator.
    let postfix = seq("bang", vec![reference("kw"), lit("!", "!")]);
    let mut kw = choice("kw", vec![postfix, lit("a", "a"), lit("b", "b")]);

    assert_eq!(
        exec_outline(&mut kw, "a!"),
        outlined("bang@0..2[a@0..1 !@1..2]", "-", "-")
    );
    assert_eq!(mode_of(&kw), Some(ChoiceMode::Pratt));
}

#[test]
fn a_reference_to_something_else_is_not_a_literal() {
    let kw = choice("kw", vec![reference("word"), lit("else", "else")]);
    let mut scope = context("scope", kw, vec![re("word", "[a-z]+")]);

    assert_eq!(
        exec_outline(&mut scope, "if"),
        outlined("word@0..2", "-", "-")
    );
    let Pattern::Context(context) = &scope else {
        unreachable!()
    };
    assert_eq!(mode_of(context.child()), Some(ChoiceMode::Ordered));
}

#[test]
fn ordered_never_takes_the_literal_set_mode_even_through_references() {
    let kw: Pattern = Choice::ordered("kw", vec![reference("if-kw"), lit("else", "else")])
        .unwrap()
        .into();
    let mut scope = context("scope", kw, vec![lit("if-kw", "if")]);

    assert!(scope.test("if"));
    let Pattern::Context(context) = &scope else {
        unreachable!()
    };
    assert_eq!(mode_of(context.child()), Some(ChoiceMode::Ordered));
}

#[test]
fn a_clone_keeps_the_literal_set() {
    let original = keyword_choice();
    let mut clone = original.clone_named("words");
    fresh_log();

    assert_eq!(mode_of(&clone), Some(ChoiceMode::LiteralSet));
    assert!(clone.test("for"));
    assert_eq!(parse_log::take_literals(), ["for"]);
}

// ─── Differential: literal set against the ordered loop ─────────────────

/// Everything a caller can observe after a parse. The two modes are built
/// separately, so the choices (and anything around them) have different
/// ids; patterns are told apart by name, and the differential gives every
/// literal a unique one (`numbered`) so a duplicate token recorded under
/// the wrong literal can't pass for the right one.
#[derive(Debug, PartialEq)]
struct Observed {
    tree: String,
    end: usize,
    error: String,
    furthest: String,
    leaf: String,
}

fn error_text(error: Option<ParseError>) -> String {
    error.map_or_else(
        || "-".to_string(),
        |e| format!("{}@{}..{}", e.pattern.name, e.start, e.last),
    )
}

fn leaf_text(leaf: Option<Match>) -> String {
    leaf.map_or_else(
        || "-".to_string(),
        |m| format!("{}@{}..{}", m.pattern.name, m.start, m.end),
    )
}

/// `parse` from 0, as a caller driving the cursor sees it.
fn observe_parse(pattern: &mut Pattern, text: &str) -> Observed {
    observe_parse_at(pattern, text, 0)
}

/// `parse` from `start`.
fn observe_parse_at(pattern: &mut Pattern, text: &str, start: usize) -> Observed {
    let mut cursor = Cursor::new(text);
    cursor.set_pos(start);
    let node = pattern.parse(&mut cursor);
    let end = cursor.pos();
    let error = error_text(cursor.error());
    let furthest = error_text(cursor.furthest_error());
    let leaf = leaf_text(cursor.leaf_match());
    let tree = node.map_or_else(
        || "-".to_string(),
        |node| outline(cursor.into_tree(node).root()),
    );
    Observed {
        tree,
        end,
        error,
        furthest,
        leaf,
    }
}

fn observe_exec(pattern: &mut Pattern, text: &str) -> Observed {
    let result = pattern.exec(text);
    assert!(result.pattern_error.is_none());
    Observed {
        tree: result
            .tree()
            .map_or_else(|| "-".to_string(), |tree| outline(tree.root())),
        end: result.tree().map_or(0, |tree| tree.root().end()),
        error: error_text(result.error),
        furthest: error_text(result.furthest_error),
        leaf: leaf_text(result.leaf_match),
    }
}

/// The choices compared: shared prefixes, a longer literal after a shorter
/// one with the same start, duplicates, multi-byte and emoji literals,
/// literals sharing only a UTF-8 lead byte or a high surrogate, a
/// decomposed accent, and a single literal.
const CHOICES: &[&[&str]] = &[
    &KEYWORDS,
    &["ab", "a", "abc", "abd", "b", "abc", "a"],
    &["abc", "ab", "a"],
    &["é", "è", "e\u{301}", "e", "ñ", "😀a", "😀", "😁", "😀😀"],
    &["hello"],
    &["x", "xy", "xyz", "y"],
];

/// Literals named after their position too: `0:ab`, `1:a`, … so
/// duplicates are distinct.
fn numbered(tokens: &[&str]) -> Vec<Pattern> {
    tokens
        .iter()
        .enumerate()
        .map(|(i, t)| lit(&format!("{i}:{t}"), t))
        .collect()
}

/// `""`, then for every token of every choice: each prefix, and each
/// one-character mutation (a character replaced, deleted or inserted,
/// anywhere, with any character of the tokens or a few strangers).
fn inputs() -> Vec<String> {
    let mut alphabet: Vec<char> = CHOICES
        .iter()
        .flat_map(|tokens| tokens.iter().flat_map(|t| t.chars()))
        .collect();
    alphabet.extend(['z', ';', '\u{301}', '\u{1F602}']);
    alphabet.sort_unstable();
    alphabet.dedup();

    let mut inputs = vec![String::new()];
    for token in CHOICES.iter().flat_map(|tokens| tokens.iter()) {
        let chars: Vec<char> = token.chars().collect();
        for i in 0..=chars.len() {
            inputs.push(chars[..i].iter().collect());
            for &c in &alphabet {
                let mut inserted = chars.clone();
                inserted.insert(i, c);
                inputs.push(inserted.into_iter().collect());
                if i < chars.len() {
                    let mut replaced = chars.clone();
                    replaced[i] = c;
                    inputs.push(replaced.into_iter().collect());
                }
            }
            if i < chars.len() {
                let mut deleted = chars.clone();
                deleted.remove(i);
                inputs.push(deleted.into_iter().collect());
            }
        }
    }
    inputs.sort();
    inputs.dedup();
    inputs
}

#[test]
fn the_literal_set_matches_the_ordered_loop_on_every_nearby_input() {
    let inputs = inputs();
    assert!(inputs.len() > 1000, "{} inputs", inputs.len());
    let mut compared = 0;

    for tokens in CHOICES {
        // Built once and reused for every input, as a grammar is.
        let (mut fast, mut plain) = both("choice", numbered(tokens));
        // The choice after something, from a position other than 0.
        let (inner_fast, inner_plain) = both("choice", numbered(tokens));
        let mut after_fast = seq("after", vec![lit("lt", "<"), inner_fast]);
        let mut after_plain = seq("after", vec![lit("lt", "<"), inner_plain]);
        // A repeat reports its own `min` failure where the last attempt
        // stopped, so it sees what a failed choice leaves behind.
        let (twice_fast, twice_plain) = both("choice", numbered(tokens));
        let twice = |child| {
            rep(
                "twice",
                child,
                RepeatOptions {
                    min: 2,
                    ..RepeatOptions::default()
                },
            )
        };
        let mut twice_fast = twice(twice_fast);
        let mut twice_plain = twice(twice_plain);

        for text in &inputs {
            let after = format!("<{text}");
            assert_eq!(
                observe_parse(&mut fast, text),
                observe_parse(&mut plain, text),
                "parse {tokens:?} on {text:?}"
            );
            assert_eq!(
                observe_exec(&mut fast, text),
                observe_exec(&mut plain, text),
                "exec {tokens:?} on {text:?}"
            );
            assert_eq!(
                observe_parse(&mut after_fast, &after),
                observe_parse(&mut after_plain, &after),
                "after {tokens:?} on {after:?}"
            );
            assert_eq!(
                observe_parse_at(&mut fast, text, text.len()),
                observe_parse_at(&mut plain, text, text.len()),
                "at the end, {tokens:?} on {text:?}"
            );
            assert_eq!(
                observe_parse(&mut twice_fast, text),
                observe_parse(&mut twice_plain, text),
                "twice {tokens:?} on {text:?}"
            );
            compared += 1;
        }
    }

    assert_eq!(compared, CHOICES.len() * inputs.len());
}

#[test]
fn literals_through_references_match_the_ordered_loop() {
    // `kw = if-kw | "else" | int-kw | "i"`, inside a statement. One set of
    // patterns, cloned into both, so their ids agree.
    let alternatives = vec![
        reference("if-kw"),
        lit("else", "else"),
        reference("int-kw"),
        lit("i", "i"),
    ];
    let end = lit("end", ";");
    let definitions = vec![lit("if-kw", "if"), lit("int-kw", "int")];
    let build = |kw: Choice| {
        let root = seq("root", vec![kw.into(), end.clone()]);
        context("scope", root, definitions.clone())
    };
    let mut fast = build(Choice::new("kw", alternatives.clone()).unwrap());
    let mut plain = build(Choice::ordered("kw", alternatives).unwrap());

    for text in [
        "", "i", "i;", "if;", "in;", "int;", "inx;", "el;", "else;", "elsx;", "x;", ";",
    ] {
        assert_eq!(
            observe_exec(&mut fast, text),
            observe_exec(&mut plain, text),
            "{text:?}"
        );
    }
    assert_eq!(kw_of(&fast).mode(), Some(ChoiceMode::LiteralSet));
    assert_eq!(kw_of(&plain).mode(), Some(ChoiceMode::Ordered));
}

#[test]
fn deviation_at_the_depth_limit_the_index_never_parses_a_losing_reference() {
    // `scope { root = kw ";" }`, `kw = if-kw | "else"`: context, sequence
    // and choice take three frames. With room for no more, the loop halts
    // on the reference's frame before reaching "else"; the index goes
    // straight to the literal, which needs no frame (DESIGN §13).
    let alternatives = vec![reference("if-kw"), lit("else", "else")];
    let build = |kw: Choice| {
        let root = seq("root", vec![kw.into(), lit("end", ";")]);
        context("scope", root, vec![lit("if-kw", "if")])
    };
    let mut fast = build(Choice::new("kw", alternatives.clone()).unwrap());
    let mut plain = build(Choice::ordered("kw", alternatives).unwrap());
    let limits = Limits { max_depth: 3 };

    let fast = fast.exec_with("else;", limits);
    let plain = plain.exec_with("else;", limits);

    assert_eq!(fast.tree().unwrap().root().value(), "else;");
    assert!(fast.too_deep.is_none());
    assert!(plain.tree().is_none());
    assert!(plain.too_deep.is_some());
}
