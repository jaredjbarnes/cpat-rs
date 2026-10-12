//! Introspection of the leaves, ported from v12 `Literal.test.ts`,
//! `Regex.test.ts`, `TakeUntil.test.ts`, `Cut.test.ts` and
//! `BasePattern.test.ts` (each test names its source). A v12 test that gives
//! the pattern a parent asks about a path inside a root; a "With Null
//! Parent" test asks about the root itself (the empty path).
//!
//! `find` returns the instance with its path, where v12 returns the instance
//! that knows its parent; asking "what comes next" of it takes the path.

mod common;

use common::*;
use cpat_core::{
    Cursor, Custom, CustomParse, Meta, NodeId, NodeKind, Pattern, PatternPath, Regex, TakeUntil,
    Tokenizable,
};

// ─── Literal.test.ts ───

#[test]
fn literal_clone() {
    // Literal.test.ts 'Clone'
    let literal = lit("greeting", "Hello World!");
    let clone = literal.clone_named("greeting");

    assert_eq!(clone.name(), "greeting");
    assert_eq!(clone.id(), literal.id());
}

#[test]
fn literal_get_tokens() {
    // Literal.test.ts 'Get Tokens'
    assert_eq!(lit("a", "A").tokens(), ["A"]);
}

#[test]
fn literal_get_tokens_after() {
    // Literal.test.ts 'Get Tokens After': a literal has no children.
    assert!(lit("a", "A").tokens_after(0).is_empty());
}

#[test]
fn literal_properties() {
    // Literal.test.ts 'Properties'
    let literal = lit("a", "A");

    assert_eq!(literal.type_name(), "literal");
    assert_eq!(literal.name(), "a");
    assert!(literal.introspect().at(&at(&[0])).is_none(), "no children");
}

#[test]
fn literal_get_next_tokens() {
    // Literal.test.ts 'Get Next Tokens'
    let parent = seq(
        "parent",
        vec![seq("sequence", vec![lit("a", "A")]), lit("b", "B")],
    );
    let a = path_of(&parent, "a");

    assert_eq!(parent.next_tokens(&a), ["B"]);
}

#[test]
fn literal_get_next_tokens_with_null_parent() {
    // Literal.test.ts 'Get Next Tokens With Null Parent'
    assert!(lit("a", "A").next_tokens(&PatternPath::root()).is_empty());
}

#[test]
fn literal_get_patterns() {
    // Literal.test.ts 'Get Patterns': a leaf is its own pattern.
    let a = lit("a", "A");
    let patterns = a.patterns();

    assert_eq!(patterns.len(), 1);
    assert!(std::ptr::eq(patterns[0].pattern, &a));
    assert_eq!(paths(&patterns), ["/"]);
}

#[test]
fn literal_get_next_patterns() {
    // Literal.test.ts 'Get Next Patterns'
    let parent = seq(
        "parent",
        vec![seq("sequence", vec![lit("a", "A")]), lit("b", "B")],
    );
    let a = path_of(&parent, "a");
    let b = parent.find(|p| p.name() == "b").unwrap();

    let next = parent.next_patterns(&a);

    assert_eq!(next.len(), 1);
    assert!(std::ptr::eq(next[0].pattern, b.pattern));
    assert_eq!(next[0].path, b.path);
}

#[test]
fn literal_get_next_patterns_with_null_parent() {
    // Literal.test.ts 'Get Next Patterns With Null Parent'
    assert!(lit("a", "A").next_patterns(&PatternPath::root()).is_empty());
}

#[test]
fn literal_get_patterns_after() {
    // Literal.test.ts 'Get Patterns After'
    assert!(lit("a", "A").patterns_after(0).is_empty());
}

#[test]
fn literal_find_pattern() {
    // Literal.test.ts 'Find Pattern'. A literal's own `find` answers nothing,
    // not even itself (v12 `Literal.find` returns null).
    let a = lit("a", "A");
    assert!(a.find(|p| p.name() == "nada").is_none());
    assert!(a.find(|p| p.name() == "a").is_none());
}

// ─── Regex.test.ts ───

#[test]
fn regex_get_tokens() {
    // Regex.test.ts 'Get Tokens'
    let mut a = Regex::new("a", "A").unwrap();
    a.set_tokens(vec!["A".into()]);

    assert_eq!(Pattern::from(a).tokens(), ["A"]);
}

#[test]
fn regex_get_tokens_after() {
    // Regex.test.ts 'Get Tokens After'
    assert!(re("a", "A").tokens_after(0).is_empty());
}

#[test]
fn regex_properties() {
    // Regex.test.ts 'Properties'
    let regex = re("a", "A");

    assert_eq!(regex.type_name(), "regex");
    assert_eq!(regex.name(), "a");
    assert!(regex.introspect().at(&at(&[0])).is_none(), "no children");
}

#[test]
fn regex_get_next_tokens() {
    // Regex.test.ts 'Get Next Tokens'
    let parent = seq("parent", vec![re("a", "A"), lit("b", "B")]);
    let a = path_of(&parent, "a");

    assert_eq!(parent.next_tokens(&a), ["B"]);
}

#[test]
fn regex_get_next_tokens_with_null_parent() {
    // Regex.test.ts 'Get Next Tokens With Null Parent'
    assert!(re("a", "A").next_tokens(&PatternPath::root()).is_empty());
}

#[test]
fn regex_get_patterns() {
    // Regex.test.ts 'Get Patterns'
    let a = re("a", "A");
    let patterns = a.patterns();

    assert_eq!(patterns.len(), 1);
    assert!(std::ptr::eq(patterns[0].pattern, &a));
}

#[test]
fn regex_get_patterns_after() {
    // Regex.test.ts 'Get Patterns After'
    assert!(re("a", "A").patterns_after(0).is_empty());
}

#[test]
fn regex_find_pattern() {
    // Regex.test.ts 'Find Pattern'
    let a = re("a", "A");
    assert!(a.find(|p| p.name() == "other").is_none());
    assert!(a.find(|p| p.name() == "a").is_none(), "not even itself");
}

#[test]
fn regex_get_next_patterns() {
    // Regex.test.ts 'Get Next Patterns'
    let parent = seq("parent", vec![re("a", "A"), lit("b", "B")]);
    let a = path_of(&parent, "a");
    let b = parent.find(|p| p.name() == "b").unwrap();

    let next = parent.next_patterns(&a);

    assert_eq!(next.len(), 1);
    assert!(std::ptr::eq(next[0].pattern, b.pattern));
}

#[test]
fn regex_get_next_patterns_with_null_parent() {
    // Regex.test.ts 'Get Next Patterns With Null Parent'
    assert!(re("a", "A").next_patterns(&PatternPath::root()).is_empty());
}

#[test]
fn a_regex_clone_keeps_its_tokens() {
    // Regex.ts `clone` copies `_tokens`.
    let regex = re_tokens("a", "A", &["A", "B"]);
    assert_eq!(regex.clone_named("b").tokens(), ["A", "B"]);
}

// ─── TakeUntil.test.ts ───

#[test]
fn take_until_id_and_type() {
    // TakeUntil.test.ts 'id and type accessors'
    let pattern = take_until("content", lit("end", ";"));
    assert_eq!(pattern.type_name(), "take-until");
    assert_eq!(pattern.name(), "content");
}

#[test]
fn take_until_set_tokens_and_get_tokens() {
    // TakeUntil.test.ts 'setTokens and getTokens for intellisense'
    let mut pattern = take_until("body", lit("end", "</end"));
    assert!(pattern.tokens().is_empty());

    let tokenizable = pattern
        .tokenizable_mut()
        .expect("a take-until takes tokens");
    tokenizable.set_tokens(vec!["<div>".into(), "<span>".into(), "<p>".into()]);

    assert_eq!(pattern.tokens(), ["<div>", "<span>", "<p>"]);
}

#[test]
fn take_until_is_equal() {
    // TakeUntil.test.ts 'isEqual compares two TakeUntil patterns'
    let a = take_until("body", lit("end", "</end"));
    let b = take_until("body", lit("end", "</end"));
    let c = take_until("body", lit("end", "</other"));

    assert!(a.is_equal(&b));
    assert!(!a.is_equal(&c));
}

#[test]
fn take_until_get_patterns_returns_self() {
    // TakeUntil.test.ts 'getPatterns returns self'
    let pattern = take_until("body", lit("end", ";"));
    let patterns = pattern.patterns();

    assert_eq!(patterns.len(), 1);
    assert!(std::ptr::eq(patterns[0].pattern, &pattern));
}

#[test]
fn take_until_find_always_returns_none() {
    // TakeUntil.test.ts 'find always returns null (leaf-like pattern)'
    let pattern = take_until("body", lit("end", ";"));
    assert!(pattern.find(|p| p.name() == "body").is_none());
    assert!(pattern.find(|p| p.name() == "end").is_none());
}

#[test]
fn take_until_standalone_introspection_is_empty() {
    // TakeUntil.test.ts 'standalone introspection methods return empty (no
    // parent)'. Its terminator (child 0) is never what comes next.
    let pattern = take_until("body", lit("end", ";"));

    assert!(pattern.tokens_after(0).is_empty());
    assert!(pattern.next_tokens(&PatternPath::root()).is_empty());
    assert!(pattern.patterns_after(0).is_empty());
    assert!(pattern.next_patterns(&PatternPath::root()).is_empty());
}

#[test]
fn take_until_introspection_delegates_to_parent_in_a_sequence() {
    // TakeUntil.test.ts 'introspection delegates to parent when in a sequence'
    let element = seq(
        "element",
        vec![take_until("body", lit("end", ";")), lit("close", "</tag>")],
    );

    assert_eq!(element.next_tokens(&at(&[0])), ["</tag>"]);
    assert_eq!(names(&element.next_patterns(&at(&[0]))), ["close"]);
}

#[test]
fn take_until_inside_a_sequence_offers_its_tokens() {
    // Checked against v12: the sequence starts with the take-until's tokens,
    // and its terminator's next tokens are none (v12 `getTokensAfter` is []).
    let mut body = TakeUntil::new("body", lit("end", ";"));
    body.set_tokens(vec!["<div>".into(), "<p>".into()]);
    let element = seq("element", vec![body.into(), lit("close", "</tag>")]);

    assert_eq!(element.tokens(), ["<div>", "<p>"]);
    assert!(element.next_tokens(&at(&[0, 0])).is_empty());
}

#[test]
fn take_until_clone_with_a_new_name() {
    // TakeUntil.test.ts 'clone with a new name' (also in take_until.rs) and
    // 'clone creates independent copy with same name'.
    let pattern = take_until("script-content", lit("close", "</script"));
    let same = pattern.clone_named("script-content");
    let renamed = pattern.clone_named("style-content");

    assert_eq!(same.name(), "script-content");
    assert_eq!(same.id(), pattern.id());
    assert_eq!(renamed.name(), "style-content");
}

// ─── Cut.test.ts ───

#[test]
fn cut_clone_preserves_type_and_id() {
    // Cut.test.ts 'clone preserves the cut (Reference resolution path)'
    let cut = cut();
    let clone = cut.clone_named("cut");

    assert_eq!(clone.type_name(), "cut");
    assert_eq!(clone.id(), cut.id());
}

#[test]
fn a_cut_offers_what_follows_it() {
    // Checked against v12 (`Cut.getTokens` is `getNextTokens`): a cut
    // consumes nothing, so a sequence led by one offers what follows it,
    // and stops there (a cut is neither optional nor a lookahead).
    let sequence = seq("s", vec![cut(), lit("b", "B"), lit("c", "C")]);

    assert_eq!(sequence.tokens(), ["B"]);
    assert_eq!(names(&sequence.patterns()), ["b"]);
    assert_eq!(sequence.next_tokens(&at(&[0])), ["B"]);
}

// ─── BasePattern.test.ts ───

/// A custom pattern that matches one `c`.
#[derive(Clone)]
struct OneC;

impl CustomParse for OneC {
    fn parse(&mut self, meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let start = cursor.pos();
        if !cursor.rest().starts_with('c') {
            cursor.record_error(start, start, meta);
            return None;
        }
        let node = cursor.leaf(NodeKind::Custom, meta.name, start, start + 1);
        cursor.record_match(meta, start, start + 1);
        cursor.set_pos(start + 1);
        Some(node)
    }

    fn clone_box(&self) -> Box<dyn CustomParse> {
        Box::new(self.clone())
    }
}

fn custom(name: &str) -> Pattern {
    Custom::new(name, Box::new(OneC)).into()
}

#[test]
fn base_a_custom_pattern_satisfies_the_pattern_contract() {
    // BasePattern.test.ts 'a subclass satisfies the Pattern interface'
    let pattern = custom("custom");

    assert_eq!(pattern.type_name(), "custom");
    assert_eq!(pattern.name(), "custom");
    assert!(pattern.introspect().at(&at(&[0])).is_none(), "no children");
    assert!(pattern.tokens().is_empty());
    assert_eq!(pattern.patterns().len(), 1, "a leaf is its own pattern");
}

#[test]
fn base_clone_carries_the_id_across() {
    // BasePattern.test.ts 'clone carries the id across, which recursion
    // detection depends on'
    let original = custom("a");
    let clone = original.clone_named("b");

    assert_eq!(clone.id(), original.id());
    assert_eq!(clone.name(), "b");
}

#[test]
fn base_get_next_tokens_is_empty_without_a_parent() {
    // BasePattern.test.ts 'getNextTokens returns empty without a parent'
    assert!(custom("a").next_tokens(&PatternPath::root()).is_empty());
}

#[test]
fn base_get_next_patterns_is_empty_without_a_parent() {
    // BasePattern.test.ts 'getNextPatterns returns empty without a parent'
    assert!(custom("a").next_patterns(&PatternPath::root()).is_empty());
}

#[test]
fn base_get_next_tokens_delegates_to_the_parent() {
    // BasePattern.test.ts 'getNextTokens delegates to the parent when there
    // is one'
    let sequence = seq("sequence", vec![lit("a", "A"), lit("b", "B")]);
    assert_eq!(sequence.next_tokens(&at(&[0])), ["B"]);
}

#[test]
fn base_a_custom_pattern_composes_with_the_built_ins() {
    // BasePattern.test.ts 'a custom pattern composes with the built-ins'
    let mut sequence = seq("sequence", vec![lit("a", "A"), custom("custom")]);
    let custom = sequence.introspect().at(&at(&[1])).unwrap();

    assert_eq!(custom.pattern.type_name(), "custom");
    assert_eq!(sequence.next_tokens(&at(&[0])), Vec::<String>::new());
    assert!(sequence.test("Ac"));
}

#[test]
fn a_custom_pattern_offers_the_tokens_it_was_given() {
    // v12's custom patterns write their own `getTokens`; a Rust one is told
    // its tokens, like a regex.
    let mut pattern = custom("c");
    pattern
        .tokenizable_mut()
        .expect("a custom pattern takes tokens")
        .set_tokens(vec!["c".into()]);
    let sequence = seq("s", vec![lit("a", "A"), pattern]);

    assert_eq!(sequence.next_tokens(&at(&[0])), ["c"]);
    assert_eq!(sequence.find(|p| p.name() == "c").unwrap().path, at(&[1]));
}

// ─── Tokenizable ───

#[test]
fn set_tokens_works_on_regex_take_until_and_custom_only() {
    let mut tokenizable = [re("r", "x"), take_until("t", lit("e", ";")), custom("c")];
    for pattern in &mut tokenizable {
        let name = pattern.name();
        pattern
            .tokenizable_mut()
            .unwrap_or_else(|| panic!("{name} takes tokens"))
            .set_tokens(vec!["tok".into()]);
        assert_eq!(pattern.tokens(), ["tok"], "{name}");
    }

    // A literal's tokens are its text and a sequence's its children's:
    // neither offers the capability (and `Literal`, `Sequence` don't
    // implement `Tokenizable`, so a typed call doesn't compile).
    let mut others = [
        lit("l", "x"),
        seq("s", vec![lit("l", "x")]),
        choice("c", vec![lit("l", "x")]),
        opt("o", lit("l", "x")),
    ];
    for pattern in &mut others {
        assert!(pattern.tokenizable_mut().is_none(), "{}", pattern.name());
    }
    let _: &dyn Tokenizable = &Regex::new("r", "x").unwrap();
    let _: &dyn Tokenizable = &TakeUntil::new("t", lit("e", ";"));
}
