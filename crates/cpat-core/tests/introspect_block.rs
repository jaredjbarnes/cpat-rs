//! Introspection of blocks, ported from v12 `Block.test.ts` ('clone',
//! 'metadata', 'autocomplete introspection' and 'equality'; each test names
//! its source). A block's children are `[open, content, close]`, or
//! `[open, close]` without content.

mod common;

use common::*;
use cpat_core::PatternPath;

#[test]
fn clone_creates_an_independent_copy() {
    // Block.test.ts 'clone' › 'clone creates independent copy'
    let pattern = block("braces", "{", Some(re("content", "[^{}]+")), "}");
    let clone = pattern.clone_named("braces-copy");

    assert_eq!(clone.name(), "braces-copy");
    assert_eq!(clone.type_name(), "block");
    assert_eq!(clone.id(), pattern.id());
}

#[test]
fn metadata_type_and_name() {
    // Block.test.ts 'metadata' › 'has correct type', 'has correct name'
    let pattern = block("my-block", "{", None, "}");

    assert_eq!(pattern.type_name(), "block");
    assert_eq!(pattern.name(), "my-block");
}

#[test]
fn metadata_children_include_open_content_and_close() {
    // Block.test.ts 'metadata' › 'children includes open, content, and close
    // patterns' and 'children without content has open and close only'
    let with = block("braces", "{", Some(re("content", "[^{}]+")), "}");
    let without = block("braces", "{", None, "}");
    let child = |p: &cpat_core::Pattern, i: u32| p.introspect().at(&at(&[i])).map(|h| h.name());

    assert_eq!(
        [
            child(&with, 0),
            child(&with, 1),
            child(&with, 2),
            child(&with, 3)
        ],
        [Some("open"), Some("content"), Some("close"), None]
    );
    assert_eq!(
        [child(&without, 0), child(&without, 1), child(&without, 2)],
        [Some("open"), Some("close"), None]
    );
}

#[test]
fn metadata_get_tokens_returns_the_open_token() {
    // Block.test.ts 'metadata' › 'getTokens returns open pattern tokens'
    assert_eq!(block("braces", "{", None, "}").tokens(), ["{"]);
}

#[test]
fn suggests_content_tokens_after_the_open_delimiter() {
    // Block.test.ts 'autocomplete introspection' › 'suggests content tokens
    // after open delimiter'
    let pattern = block("config", "{", Some(lit("keyword", "key")), "}");
    assert_eq!(pattern.tokens_after(0), ["key"]);
}

#[test]
fn suggests_the_close_token_after_content() {
    // Block.test.ts 'autocomplete introspection' › 'suggests close token
    // after content'
    let pattern = block("config", "{", Some(lit("keyword", "key")), "}");
    assert_eq!(pattern.tokens_after(1), ["}"]);
}

#[test]
fn suggests_optional_content_and_close_after_open() {
    // Block.test.ts 'autocomplete introspection' › 'suggests both optional
    // content and close after open'
    let pattern = block(
        "maybe",
        "{",
        Some(opt("maybe-digits", re("digits", "[0-9]+"))),
        "}",
    );
    let patterns = pattern.patterns_after(0);

    assert_eq!(patterns.len(), 2);
    assert_eq!(patterns[0].pattern.type_name(), "optional");
    assert_eq!(patterns[1].name(), "close");
}

#[test]
fn after_close_delegates_to_the_parent_sequence() {
    // Block.test.ts 'autocomplete introspection' › 'after close delimiter
    // delegates to parent sequence'
    let statement = seq(
        "statement",
        vec![
            block("braces", "{", Some(re("content", "[^{}]+")), "}"),
            lit("semi", ";"),
        ],
    );
    let patterns = statement.introspect().patterns_after(&at(&[0]), 2);

    assert_eq!(names(&patterns), ["semi"]);
}

#[test]
fn a_block_in_a_sequence_knows_what_comes_next() {
    // Block.test.ts 'autocomplete introspection' › 'block in sequence knows
    // what comes next'
    let statement = seq(
        "statement",
        vec![block("braces", "{", None, "}"), lit("semi", ";")],
    );

    assert_eq!(statement.next_tokens(&at(&[0])), [";"]);
    assert_eq!(names(&statement.next_patterns(&at(&[0]))), ["semi"]);
}

#[test]
fn a_standalone_block_has_no_next_tokens_or_patterns() {
    // Block.test.ts 'autocomplete introspection' › 'standalone block has no
    // next tokens or patterns'
    let pattern = block("braces", "{", None, "}");

    assert!(pattern.next_tokens(&PatternPath::root()).is_empty());
    assert!(pattern.next_patterns(&PatternPath::root()).is_empty());
}

#[test]
fn get_patterns_returns_the_open_delimiter() {
    // Block.test.ts 'autocomplete introspection' › 'getPatterns returns the
    // open delimiter patterns'
    let pattern = block("braces", "{", None, "}");
    assert_eq!(names(&pattern.patterns()), ["open"]);
}

#[test]
fn returns_empty_for_an_unknown_child() {
    // Block.test.ts 'autocomplete introspection' › 'returns empty for unknown
    // child reference'
    assert!(block("braces", "{", None, "}").patterns_after(2).is_empty());
}

#[test]
fn find_locates_a_child_within_the_block() {
    // Block.test.ts 'autocomplete introspection' › 'find locates a child
    // pattern within the block'
    let pattern = block("braces", "{", Some(re("content", "[^{}]+")), "}");
    let found = pattern.find(|p| p.name() == "close").unwrap();

    assert_eq!(found.pattern.type_name(), "literal");
    assert_eq!(found.path, at(&[2]));
}

#[test]
fn the_last_child_of_a_block_without_a_parent_offers_nothing() {
    // v12 `Block.getPatternsAfter`: past the last child with no parent, the
    // sibling loop has nothing to run over.
    let pattern = block("braces", "{", Some(lit("keyword", "key")), "}");
    assert!(pattern.tokens_after(2).is_empty());
}

#[test]
fn a_recursive_block_offers_its_open_delimiter_inside() {
    // `braces = "{" braces? "}"`. Checked against v12.
    let root = context(
        "defs",
        reference("braces"),
        vec![block(
            "braces",
            "{",
            Some(opt("inner", reference("braces"))),
            "}",
        )],
    );
    assert_eq!(root.next_tokens(&at(&[0, 0, 0])), ["{", "}"]);
}

#[test]
fn equality_identical_structure() {
    // Block.test.ts 'equality' › 'blocks with identical structure are equal'
    let a = block("braces", "{", Some(re("content", "[^{}]+")), "}");
    let b = block("braces", "{", Some(re("content", "[^{}]+")), "}");
    assert!(a.is_equal(&b));
}

#[test]
fn equality_different_delimiters() {
    // Block.test.ts 'equality' › 'blocks with different delimiters are not
    // equal'
    let braces = block("braces", "{", None, "}");
    let parens = block("parens", "(", None, ")");
    assert!(!braces.is_equal(&parens));
}

#[test]
fn after_the_close_of_a_block_without_content_comes_what_follows_it() {
    // Without content the close is child 1, and the last child. Checked
    // against v12.
    let statement = seq(
        "statement",
        vec![block("braces", "{", None, "}"), lit("semi", ";")],
    );
    let intro = statement.introspect();

    assert_eq!(intro.tokens_after(&at(&[0]), 0), ["}"]);
    assert_eq!(intro.tokens_after(&at(&[0]), 1), [";"]);
}
