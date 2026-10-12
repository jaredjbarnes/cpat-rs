//! Ported from v12 `TakeUntil.test.ts`. Its introspection tests
//! (`getTokensAfter`, `getPatterns`, `find`, `getNextTokens`, the parent
//! getter) and `isEqual` belong to the intellisense milestone (M8). Cursor
//! positions are half-open bytes: v12's inclusive index L after a match is
//! `pos` L+1 here. Values not in v12's tests were taken from running v12 on
//! the same pattern and input.

mod common;

use common::*;
use cpat_core::{Cursor, NodeKind, Pattern, TakeUntil};

fn script_content() -> Pattern {
    take_until("script-content", lit("close-script-tag", "</script"))
}

/// Parses from 0: the node's value and kind, and where the cursor ended.
fn parsed(pattern: &mut Pattern, text: &str) -> (Option<(String, NodeKind)>, usize) {
    let mut cursor = Cursor::new(text);
    let node = pattern.parse(&mut cursor);
    let pos = cursor.pos();
    let node = node.map(|node| {
        let tree = cursor.into_tree(node);
        let root = tree.root();
        (root.value().to_string(), root.kind())
    });
    (node, pos)
}

#[test]
fn take_with_no_end() {
    let result = script_content().exec("function(){}");

    assert_eq!(result.tree().unwrap().root().value(), "function(){}");
}

#[test]
fn take_until_terminating_match() {
    let (node, pos) = parsed(&mut script_content(), "function(){}function(){}</script");

    let (value, kind) = node.unwrap();
    assert_eq!(value, "function(){}function(){}");
    assert_eq!(kind, NodeKind::TakeUntil);
    // v12: index 23, on the last character taken.
    assert_eq!(pos, 24);
}

#[test]
fn take_until_terminating_complex_match() {
    let mut pattern = take_until(
        "script-content",
        choice(
            "end-tags",
            vec![
                lit("close-script-tag", "</script"),
                lit("close-style-tag", "</style"),
            ],
        ),
    );

    for text in [
        "function(){}function(){}</script",
        "function(){}function(){}</style",
    ] {
        let (node, pos) = parsed(&mut pattern, text);
        assert_eq!(node.unwrap().0, "function(){}function(){}", "{text}");
        assert_eq!(pos, 24, "{text}");
    }
}

#[test]
fn error_when_the_terminator_matches_at_the_start() {
    let mut cursor = Cursor::new("</script");

    assert!(script_content().parse(&mut cursor).is_none());
    assert_eq!(cursor.pos(), 0);
    assert_eq!(
        error_outline(cursor.error()),
        "script-content@0..0",
        "an error at (first, first) on the take-until itself"
    );
}

#[test]
fn test_is_true_when_content_exists_before_the_terminator_and_nothing_follows() {
    let mut pattern = take_until("body", lit("end", "</end"));

    assert!(pattern.test("some content"));
    // Takes `content` only, so not all of the text.
    assert!(!pattern.test("content</end"));
}

#[test]
fn test_is_false_when_the_terminator_is_at_the_start() {
    assert!(!take_until("body", lit("end", "</end")).test("</end"));
}

#[test]
fn name_and_kind() {
    let pattern = take_until("content", lit("end", ";"));

    assert_eq!(pattern.name(), "content");
    let (node, _) = parsed(&mut pattern.clone(), "a;");
    assert_eq!(node.unwrap().1, NodeKind::TakeUntil);
}

#[test]
fn a_clone_keeps_name_and_id_and_works_on_its_own() {
    let pattern = take_until("script-content", lit("close", "</script"));
    let mut cloned = pattern.clone();

    assert_eq!(cloned.name(), "script-content");
    assert_eq!(cloned.id(), pattern.id());
    assert_eq!(
        cloned.exec("var x = 1;").tree().unwrap().root().value(),
        "var x = 1;"
    );
}

#[test]
fn a_clone_with_a_new_name() {
    let pattern = take_until("script-content", lit("close", "</script"));
    let mut renamed = pattern.renamed("style-content");

    assert_eq!(renamed.name(), "style-content");
    assert_eq!(renamed.id(), pattern.id());
    assert_eq!(
        exec_shape(&mut renamed, "a"),
        "style-content:take-until@0..1"
    );
}

#[test]
fn set_tokens_and_tokens_for_intellisense() {
    let mut pattern = TakeUntil::new("body", lit("end", "</end"));
    assert!(pattern.tokens().is_empty());

    pattern.set_tokens(vec!["<div>".into(), "<span>".into(), "<p>".into()]);

    assert_eq!(pattern.tokens(), ["<div>", "<span>", "<p>"]);
    let clone = pattern.clone();
    assert_eq!(
        clone.tokens(),
        ["<div>", "<span>", "<p>"],
        "a clone keeps them"
    );
}

#[test]
fn node_indices_cover_the_taken_text() {
    let mut tag = seq(
        "tag",
        vec![
            lit("open", "<"),
            take_until("name", lit("close", ">")),
            lit("close", ">"),
        ],
    );

    assert_eq!(
        parse_shape(&mut tag, "<abc>").0,
        "tag:sequence@0..5[open:literal@0..1 name:take-until@1..4 close:literal@4..5]"
    );
}

#[test]
fn ending_a_sequence_the_node_indices_cover_the_taken_text() {
    let mut comment = seq(
        "comment",
        vec![lit("start", "//"), take_until("body", lit("newline", "\n"))],
    );
    let mut cursor = Cursor::new("//abc");
    let node = comment.parse(&mut cursor).unwrap();

    // v12: index 4, on the last character.
    assert_eq!(cursor.pos(), 5);
    assert_eq!(
        shape(cursor.into_tree(node).root()),
        "comment:sequence@0..5[start:literal@0..2 body:take-until@2..5]"
    );
}

#[test]
fn takes_whole_graphemes() {
    // "e" + combining acute is one grapheme of three bytes.
    let mut pattern = take_until("text", lit("semicolon", ";"));

    assert_eq!(
        parse_shape(&mut pattern, "e\u{301};").0,
        "text:take-until@0..3"
    );
}

#[test]
fn takes_whole_emoji_and_stops_at_an_emoji_terminator() {
    // Each 🔴 is four bytes (two UTF-16 units in v12, where `left` is 0..1
    // and `dot` 1..3).
    let mut pair = seq(
        "pair",
        vec![take_until("left", lit("dot", "🔴")), lit("dot", "🔴")],
    );
    assert_eq!(
        parse_shape(&mut pair, "x🔴y🔴"),
        outlined(
            "pair:sequence@0..5[left:take-until@0..1 dot:literal@1..5]",
            "-",
            "dot@0..0"
        )
    );

    let mut pattern = take_until("text", lit("semicolon", ";"));
    assert_eq!(parse_shape(&mut pattern, "🔴🟢;").0, "text:take-until@0..8");
}

#[test]
fn takes_up_to_the_end_inside_a_repeat() {
    // v12: words@0..14 with every word and space, the probe for `ws` that
    // failed at the last character as the furthest error.
    let mut words = rep(
        "words",
        take_until("word", re("ws", r"\s+")),
        divided(re("ws", r"\s+")),
    );

    assert_eq!(
        parse_shape(&mut words, "one two  three"),
        outlined(
            "words:infinite-repeat@0..14[word:take-until@0..3 ws:regex@3..4 \
             word:take-until@4..7 ws:regex@7..9 word:take-until@9..14]",
            "-",
            "ws@13..13"
        )
    );
}

#[test]
fn a_terminator_that_never_returns_a_node_takes_everything() {
    // `!a` succeeds without a node, which is not a match to stop at (v12
    // checks for a node). Its failures were resolved, but the furthest one
    // stays.
    let mut pattern = take_until("all", not("n", lit("a", "a")));

    assert_eq!(
        parse_shape(&mut pattern, "aab"),
        outlined("all:take-until@0..3", "-", "a@2..2")
    );
}

#[test]
fn an_optional_terminator_stops_where_it_matches() {
    let mut pattern = take_until("o", opt("oo", lit("semi", ";")));

    assert_eq!(
        parse_shape(&mut pattern, "a;b"),
        outlined("o:take-until@0..1", "-", "semi@0..0")
    );
    // v12: the summary error from where the take ended.
    assert_eq!(
        exec_outline(&mut pattern, "a;b"),
        outlined("-", "o@1..3", "semi@0..0")
    );
}

#[test]
fn a_successful_take_is_the_leaf_match() {
    // With no terminator in the text, no probe matched, so the only match
    // recorded is the take's own. (A terminator that matched would be
    // further along: probes record matches too, as in v12.)
    let mut cursor = Cursor::new("ab");
    let mut pattern = take_until("t", lit("semi", ";"));
    pattern.parse(&mut cursor).unwrap();

    let leaf = cursor.leaf_match().unwrap();
    assert_eq!(
        (leaf.pattern.name.as_str(), leaf.start, leaf.end),
        ("t", 0, 2)
    );
}

#[test]
fn the_terminator_is_probed_inside_the_take_until_frame_at_every_grapheme() {
    // v12 makes the take-until its terminator's parent, so the recursion
    // guards see it on the chain. Every probe starts on a new grapheme.
    let (spy, seen) = frame_spy();
    let mut pattern = take_until("t", spy);
    let id = pattern.id();

    let mut cursor = Cursor::new("aé");
    pattern.parse(&mut cursor).unwrap();

    assert_eq!(*seen.borrow(), vec![vec![(id, 0)], vec![(id, 0)]]);
}

#[test]
fn a_probe_that_meets_an_unknown_name_halts_the_parse() {
    // Driven directly, nothing checks names first; the halt must not be
    // swallowed by the probe's rewind.
    let mut pattern = take_until("t", reference("missing"));
    let mut cursor = Cursor::new("abc");

    assert!(pattern.parse(&mut cursor).is_none());
    assert!(cursor.pattern_error().is_some());
    assert_eq!(error_outline(cursor.error()), "missing@0..0");
}

#[test]
fn a_reference_finds_a_pattern_inside_the_terminator() {
    // The terminator is the take-until's child, as in v12, so `exec`'s
    // implicit scope (v12 `findPattern`) searches it.
    let mut line = seq(
        "line",
        vec![take_until("body", lit("semi", ";")), reference("semi")],
    );

    assert_eq!(
        exec_shape(&mut line, "ab;"),
        "line:sequence@0..3[body:take-until@0..2 semi:literal@2..3]"
    );
}
