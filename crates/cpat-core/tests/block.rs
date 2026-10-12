//! Ported from v12 `Block.test.ts`: 'simple blocks', 'nested blocks',
//! 'error cases', 'content patterns', 'used with cursor directly', 'clone',
//! 'metadata', 'recursive self-referencing block', 'boolean test()' and
//! 'unicode and emoji'. 'autocomplete introspection' and 'equality' belong
//! to the intellisense milestone (M8).
//!
//! Positions are half-open bytes: v12's inclusive index L after a match is
//! `pos` L+1 here. Shapes not in v12's tests were taken from running v12 on
//! the same pattern and input (UTF-16 offsets converted to bytes).

mod common;

use common::*;
use cpat_core::{Block, Cursor, Literal, NodeKind, Pattern};

fn braces(content: Option<Pattern>) -> Pattern {
    block("braces", "{", content, "}")
}

fn wildcard() -> Pattern {
    braces(None)
}

/// Parses from 0: the tree's shape (or `-`) and where the cursor ended.
fn parsed(pattern: &mut Pattern, text: &str) -> (String, usize) {
    let mut cursor = Cursor::new(text);
    let node = pattern.parse(&mut cursor);
    let pos = cursor.pos();
    let shape = node.map_or_else(|| "-".into(), |node| shape(cursor.into_tree(node).root()));
    (shape, pos)
}

fn values(pattern: &mut Pattern, text: &str) -> Option<Vec<String>> {
    let result = pattern.exec(text);
    let tree = result.tree()?;
    let mut values = vec![tree.root().value().to_string()];
    values.extend(tree.root().children().map(|c| c.value().to_string()));
    Some(values)
}

// ─── simple blocks ──────────────────────────────────────────────────────

#[test]
fn matches_an_empty_block() {
    assert_eq!(
        exec_shape(&mut wildcard(), "{}"),
        "braces:block@0..2[open:literal@0..1 close:literal@1..2]"
    );
}

#[test]
fn matches_a_block_with_content() {
    let mut pattern = braces(Some(re("content", "[^{}]+")));

    assert_eq!(
        exec_shape(&mut pattern, "{hello}"),
        "braces:block@0..7[open:literal@0..1 content:regex@1..6 close:literal@6..7]"
    );
}

#[test]
fn a_wildcard_block_captures_the_inner_text() {
    assert_eq!(
        parsed(&mut wildcard(), "{stuff inside}"),
        (
            "braces:block@0..14[open:literal@0..1 \
             braces-content:block-content@1..13 close:literal@13..14]"
                .into(),
            14
        )
    );
    assert_eq!(
        values(&mut wildcard(), "{stuff inside}").unwrap(),
        ["{stuff inside}", "{", "stuff inside", "}"]
    );
}

#[test]
fn a_wildcard_block_with_nothing_inside_has_no_content_node() {
    assert_eq!(values(&mut wildcard(), "{}").unwrap(), ["{}", "{", "}"]);
}

#[test]
fn a_wildcard_block_with_nested_delimiters() {
    assert_eq!(
        parsed(&mut wildcard(), "{ { } }").0,
        "braces:block@0..7[open:literal@0..1 braces-content:block-content@1..6 \
         close:literal@6..7]"
    );
    assert_eq!(values(&mut wildcard(), "{ { } }").unwrap()[2], " { } ");
}

#[test]
fn matches_a_block_with_whitespace_content() {
    let mut pattern = braces(Some(re("content", r"\s+")));

    assert_eq!(values(&mut pattern, "{ }").unwrap()[0], "{ }");
}

#[test]
fn matches_a_parenthesized_block() {
    let mut pattern = block("parens", "(", Some(re("content", "[^()]+")), ")");

    assert_eq!(values(&mut pattern, "(hello)").unwrap()[0], "(hello)");
}

#[test]
fn matches_a_block_with_multi_character_delimiters() {
    let mut pattern = block("begin-end", "BEGIN", Some(re("content", "[^BE]+")), "END");

    assert_eq!(
        exec_shape(&mut pattern, "BEGIN stuff END"),
        "begin-end:block@0..15[open:literal@0..5 content:regex@5..12 close:literal@12..15]"
    );
}

// ─── nested blocks ──────────────────────────────────────────────────────

#[test]
fn the_scan_finds_the_matching_close_past_nested_braces() {
    assert_eq!(parsed(&mut wildcard(), "{ { } }").1, 7);
}

#[test]
fn counts_nesting_depth() {
    assert_eq!(parsed(&mut wildcard(), "{ { { } } }").1, 11);
}

#[test]
fn nested_blocks_with_recursive_content() {
    let ws = || opt("ws", re("whitespace", r"\s+"));
    let inner = block("inner", "{", None, "}");
    let mut outer = block(
        "outer",
        "{",
        Some(seq("content", vec![ws(), inner, ws()])),
        "}",
    );

    assert_eq!(
        exec_shape(&mut outer, "{ {} }"),
        "outer:block@0..6[open:literal@0..1 content:sequence@1..5[whitespace:regex@1..2 \
         inner:block@2..4[open:literal@2..3 close:literal@3..4] whitespace:regex@4..5] \
         close:literal@5..6]"
    );
}

#[test]
fn more_opens_than_closes_ends_at_the_last_close() {
    let mut tag = block("tag", "<task>", None, "</task>");

    assert_eq!(
        parsed(&mut tag, "<task>stuff <task> more stuff</task>").0,
        "tag:block@0..36[open:literal@0..6 tag-content:block-content@6..29 \
         close:literal@29..36]"
    );
}

#[test]
fn several_unmatched_opens_end_at_the_last_close() {
    assert_eq!(parsed(&mut wildcard(), "{ { { }").1, 7);
}

#[test]
fn fails_when_there_is_no_close_at_all() {
    let mut cursor = Cursor::new("{ no close here");

    assert!(wildcard().parse(&mut cursor).is_none());
    assert_eq!(error_outline(cursor.error()), "braces@0..0");
}

#[test]
fn adjacent_blocks() {
    let mut pattern = braces(Some(re("content", "[^{}]+")));
    let mut cursor = Cursor::new("{a}{b}");

    let first = pattern.parse(&mut cursor).unwrap();
    assert_eq!((cursor.node_start(first), cursor.node_end(first)), (0, 3));
    // v12: index 2, then `next()`; here the cursor is already past it.
    assert_eq!(cursor.pos(), 3);

    let second = pattern.parse(&mut cursor).unwrap();
    assert_eq!((cursor.node_start(second), cursor.node_end(second)), (3, 6));
}

// ─── error cases ────────────────────────────────────────────────────────

#[test]
fn an_unmatched_open_fails() {
    assert_eq!(
        exec_outline(&mut wildcard(), "{"),
        outlined("-", "braces@0..1", "braces@0..0")
    );
}

#[test]
fn text_that_does_not_start_with_open_fails() {
    // The open literal's failure is the furthest; the block's own error is
    // at (first, first).
    assert_eq!(
        parse_shape(&mut wildcard(), "x{}"),
        outlined("-", "braces@0..0", "open@0..0")
    );
    assert!(!wildcard().test("x{}"));
}

#[test]
fn empty_input_fails() {
    assert!(wildcard().exec("").tree().is_none());
}

// ─── content patterns ───────────────────────────────────────────────────

#[test]
fn matches_sequence_content() {
    let mut pattern = braces(Some(seq(
        "content",
        vec![
            re("name", "[a-z]+"),
            lit("colon", ":"),
            re("value", "[a-z]+"),
        ],
    )));

    assert_eq!(
        exec_shape(&mut pattern, "{foo:bar}"),
        "braces:block@0..9[open:literal@0..1 content:sequence@1..8[name:regex@1..4 \
         colon:literal@4..5 value:regex@5..8] close:literal@8..9]"
    );
}

#[test]
fn optional_content_may_be_absent() {
    let mut pattern = braces(Some(opt("maybe", re("content", "[a-z]+"))));

    assert_eq!(
        exec_shape(&mut pattern, "{}"),
        "braces:block@0..2[open:literal@0..1 close:literal@1..2]"
    );
}

#[test]
fn optional_content_that_does_not_match_leaves_the_block_without_content() {
    // v12 keeps the block, open and close only, skipping the text between.
    let mut pattern = braces(Some(opt("maybe", re("content", "[a-z]+"))));

    assert_eq!(
        parse_shape(&mut pattern, "{1}"),
        outlined(
            "braces:block@0..3[open:literal@0..1 close:literal@2..3]",
            "-",
            "content@1..1"
        )
    );
}

#[test]
fn deviation_a_block_whose_absent_content_skipped_text_still_matches_all_of_it() {
    // v12's exec compares the joined children's values ("{}") with the
    // text ("{1}") and reports a summary error at 2. A node's value here is
    // the slice it spans (DESIGN §10), which is all of the text.
    let mut pattern = braces(Some(opt("maybe", re("content", "[a-z]+"))));

    assert_eq!(
        exec_shape(&mut pattern, "{1}"),
        "braces:block@0..3[open:literal@0..1 close:literal@2..3]"
    );
}

#[test]
fn required_content_that_does_not_match_fails() {
    let mut pattern = block("typed", "{", Some(re("digits", "[0-9]+")), "}");

    assert_eq!(
        exec_outline(&mut pattern, "{abc}"),
        outlined("-", "typed@0..5", "digits@1..1")
    );
}

#[test]
fn content_that_only_partly_fills_the_block_fails() {
    let mut pattern = block("typed", "{", Some(re("alpha", "[a-z]+")), "}");

    assert!(pattern.exec("{abc 123}").tree().is_none());
}

#[test]
fn content_that_runs_past_the_close_fails() {
    // Content parses on the whole text; it must end exactly at the close.
    let mut pattern = braces(Some(re("any", ".+")));

    assert_eq!(
        parse_shape(&mut pattern, "{ab}"),
        outlined("-", "braces@0..0", "braces@0..0")
    );
}

#[test]
fn content_that_matches_nothing_without_failing_fails_unless_optional() {
    // `!x` succeeds with no node, which only an `Optional` may do (v12
    // checks the content pattern's type).
    let mut pattern = braces(Some(not("nx", lit("x", "x"))));

    assert_eq!(
        parse_shape(&mut pattern, "{ab}"),
        outlined("-", "braces@0..0", "x@1..1")
    );
}

#[test]
fn a_cut_inside_failed_content_commits_the_choice() {
    // The block keeps no part of failed content, but it doesn't restore the
    // cut either, as in v12: the choice is committed to the block.
    let mut pattern = choice(
        "choice",
        vec![
            braces(Some(seq("ab", vec![lit("a", "a"), cut(), lit("b", "b")]))),
            re("any", ".+"),
        ],
    );

    assert_eq!(
        exec_outline(&mut pattern, "{ac}"),
        outlined("-", "choice@0..4", "b@2..2")
    );
    assert_eq!(exec_shape(&mut pattern, "{x}"), "any:regex@0..3");
}

#[test]
fn a_block_inside_a_sequence() {
    let mut statement = seq(
        "statement",
        vec![braces(Some(re("content", "[^{}]+"))), lit("semi", ";")],
    );

    assert_eq!(
        exec_shape(&mut statement, "{a};"),
        "statement:sequence@0..4[braces:block@0..3[open:literal@0..1 content:regex@1..2 \
         close:literal@2..3] semi:literal@3..4]"
    );
}

#[test]
fn blocks_in_a_repeat() {
    let mut blocks = rep("blocks", wildcard(), divided(lit("comma", ",")));

    assert_eq!(
        exec_shape(&mut blocks, "{},{}"),
        "blocks:infinite-repeat@0..5[braces:block@0..2[open:literal@0..1 close:literal@1..2] \
         comma:literal@2..3 braces:block@3..5[open:literal@3..4 close:literal@4..5]]"
    );
}

// ─── used with cursor directly ──────────────────────────────────────────

#[test]
fn parse_advances_the_cursor_past_the_block() {
    // v12: index 1, on the close.
    assert_eq!(
        parsed(&mut wildcard(), "{} rest"),
        (
            "braces:block@0..2[open:literal@0..1 close:literal@1..2]".into(),
            2
        )
    );
}

#[test]
fn parse_restores_the_cursor_on_failure() {
    let mut cursor = Cursor::new("hello");

    assert!(wildcard().parse(&mut cursor).is_none());
    assert_eq!(cursor.pos(), 0);
}

#[test]
fn a_failure_after_the_open_rewinds_to_the_start() {
    let mut pattern = seq("s", vec![lit("x", "x"), wildcard()]);
    let mut cursor = Cursor::new("x{ab");

    assert!(pattern.parse(&mut cursor).is_none());
    assert_eq!(error_outline(cursor.error()), "braces@1..1");
}

#[test]
fn a_direct_parse_that_fails_after_the_open_leaves_the_cursor_at_the_start() {
    // The open matched and moved the cursor; the failure must undo that.
    // v12: cursor.index 0 for each (no close, content that doesn't match,
    // content that doesn't reach the close).
    for text in ["{ no close", "{a}", "{ab"] {
        let mut pattern = braces(Some(re("d", "[0-9]+")));
        let mut cursor = Cursor::new(text);

        assert!(pattern.parse(&mut cursor).is_none(), "{text}");
        assert_eq!(cursor.pos(), 0, "{text}");
    }
}

// ─── clone ──────────────────────────────────────────────────────────────

#[test]
fn a_clone_with_a_new_name_keeps_the_id_and_names_its_content_after_itself() {
    let pattern = wildcard();
    let mut renamed = pattern.renamed("braces-copy");

    assert_eq!(renamed.name(), "braces-copy");
    assert_eq!(renamed.id(), pattern.id());
    assert_eq!(
        exec_shape(&mut renamed, "{a}"),
        "braces-copy:block@0..3[open:literal@0..1 braces-copy-content:block-content@1..2 \
         close:literal@2..3]"
    );
}

// ─── metadata ───────────────────────────────────────────────────────────

#[test]
fn kind_and_name() {
    let mut pattern = block("my-block", "{", None, "}");

    assert_eq!(pattern.name(), "my-block");
    let result = pattern.exec("{}");
    assert_eq!(result.tree().unwrap().root().kind(), NodeKind::Block);
}

#[test]
fn parts_are_open_content_and_close() {
    let with = Block::new(
        "braces",
        Literal::new("open", "{").unwrap(),
        Some(re("content", "[^{}]+")),
        Literal::new("close", "}").unwrap(),
    );
    let without = Block::new(
        "braces",
        Literal::new("open", "{").unwrap(),
        None,
        Literal::new("close", "}").unwrap(),
    );

    assert_eq!(
        (
            with.open().name(),
            with.content().map(Pattern::name),
            with.close().name()
        ),
        ("open", Some("content"), "close")
    );
    assert!(without.content().is_none());
}

#[test]
fn content_that_may_be_empty() {
    let mut pattern = braces(Some(re("content", "[^{}]*")));

    assert_eq!(values(&mut pattern, "{text}").unwrap()[0], "{text}");
}

#[test]
fn sequence_content_that_refers_to_the_block() {
    let mut pattern = braces(Some(seq(
        "content",
        vec![re("content", "[^{}}]+"), opt("ref", reference("braces"))],
    )));

    assert_eq!(
        exec_shape(&mut pattern, "{text{inner}}"),
        "braces:block@0..13[open:literal@0..1 content:sequence@1..12[content:regex@1..5 \
         braces:block@5..12[open:literal@5..6 content:sequence@6..11[content:regex@6..11] \
         close:literal@11..12]] close:literal@12..13]"
    );
}

// ─── recursive self-referencing block ───────────────────────────────────

fn recursive() -> Pattern {
    block("block", "{", Some(reference("block")), "}")
}

#[test]
fn an_empty_block_is_the_base_case() {
    assert_eq!(
        exec_shape(&mut recursive(), "{}"),
        "block:block@0..2[open:literal@0..1 close:literal@1..2]"
    );
}

#[test]
fn one_level_of_nesting() {
    assert_eq!(
        values(&mut recursive(), "{{}}").unwrap(),
        ["{{}}", "{", "{}", "}"]
    );
}

#[test]
fn two_levels_of_nesting() {
    assert_eq!(
        exec_shape(&mut recursive(), "{{{}}}"),
        "block:block@0..6[open:literal@0..1 block:block@1..5[open:literal@1..2 \
         block:block@2..4[open:literal@2..3 close:literal@3..4] close:literal@4..5] \
         close:literal@5..6]"
    );
}

#[test]
fn recursion_stops_at_text_that_is_not_a_block() {
    // v12: the open literal of the inner block fails at 1.
    assert_eq!(
        exec_outline(&mut recursive(), "{hello}"),
        outlined("-", "block@0..7", "open@1..1")
    );
}

// ─── boolean test() ─────────────────────────────────────────────────────

#[test]
fn test_is_true_for_a_matching_block() {
    assert!(braces(Some(re("content", "[^{}]+"))).test("{hello}"));
}

#[test]
fn test_is_false_for_text_that_does_not_match() {
    assert!(!braces(Some(re("content", "[^{}]+"))).test("no braces here"));
}

// ─── unicode and emoji ──────────────────────────────────────────────────

#[test]
fn a_wildcard_block_with_emoji_content() {
    assert_eq!(
        exec_shape(&mut wildcard(), "{🔴🟢🔵}"),
        "braces:block@0..14[open:literal@0..1 braces-content:block-content@1..13 \
         close:literal@13..14]"
    );
}

#[test]
fn emoji_delimiters() {
    let mut stars = block("stars", "🌟", Some(re("content", "[^🌟]+")), "🌟");

    assert_eq!(
        exec_shape(&mut stars, "🌟hello🌟"),
        "stars:block@0..13[open:literal@0..4 content:regex@4..9 close:literal@9..13]"
    );
}

#[test]
fn multi_code_point_emoji_delimiters() {
    // ➡️ and ⬅️ are an arrow plus a variation selector: six bytes each.
    let mut arrows = block("arrows", "➡️", None, "⬅️");

    assert_eq!(
        exec_shape(&mut arrows, "➡️content here⬅️"),
        "arrows:block@0..24[open:literal@0..6 arrows-content:block-content@6..18 \
         close:literal@18..24]"
    );
}

#[test]
fn nested_blocks_with_emoji_between() {
    assert_eq!(
        parsed(&mut wildcard(), "{🔴{🟢}🔵}").0,
        "braces:block@0..16[open:literal@0..1 braces-content:block-content@1..15 \
         close:literal@15..16]"
    );
}

#[test]
fn combining_characters_in_the_content() {
    assert_eq!(
        values(&mut wildcard(), "{cafe\u{301}}").unwrap()[0],
        "{cafe\u{301}}"
    );
}

#[test]
fn the_cursor_is_past_an_emoji_block() {
    // v12: on the close (`currentChar` is "}").
    assert_eq!(parsed(&mut wildcard(), "{🔴} rest").1, 6);
}

#[test]
fn delimiters_longer_than_one_character() {
    let mut pattern = block("braces", "<script", None, "</script>");
    let text = "<script>Hello</script>";
    let result = pattern.exec(text);
    let root = result.tree().unwrap().root();

    assert_eq!(root.value(), text);
    // v12: the cursor on the last ">".
    assert_eq!(root.end(), text.len());
}

#[test]
fn deviation_an_open_token_ending_inside_a_grapheme_starts_the_content_after_it() {
    // "e" matches the base of "e" + combining accent. v12 moves its cursor
    // back to the grapheme's start, so its content overlaps the open and
    // its exec fails ("eéx}" is not the text). Here the content starts at
    // the next boundary, where the open literal left the cursor; a position
    // inside a grapheme doesn't exist (DESIGN §5c).
    let mut pattern = block("b", "e", Some(re("c", "[^}]+")), "}");

    assert_eq!(
        exec_shape(&mut pattern, "e\u{301}x}"),
        "b:block@0..5[open:literal@0..1 c:regex@3..4 close:literal@4..5]"
    );
}

#[test]
fn every_part_parses_inside_the_block_frame() {
    // v12 makes the block its parts' parent, so the recursion guards see it
    // on the chain. The spy is optional content: it must be reached.
    let (spy, seen) = frame_spy();
    let mut pattern = seq("s", vec![lit("x", "x"), braces(Some(opt("maybe", spy)))]);
    let outer = pattern.id();
    let mut cursor = Cursor::new("x{a}");
    pattern.parse(&mut cursor).unwrap();

    let frames = seen.borrow();
    assert_eq!(frames.len(), 1);
    let starts: Vec<_> = frames[0].iter().map(|&(_, start)| start).collect();
    assert_eq!(frames[0][0].0, outer);
    // The sequence at 0, the block at 1, the optional at 2.
    assert_eq!(starts, [0, 1, 2]);
}
