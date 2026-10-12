//! Introspection through references and contexts, ported from v12
//! `Reference.test.ts` and `Context.test.ts` (each test names its source),
//! plus resolution cases checked against v12 on the same grammars.
//!
//! A reference's child 0 is what it resolves to, followed as a parse would
//! follow it (DESIGN §16): v12 resolves it the first time it is asked.

mod common;

use common::*;
use cpat_core::{Pattern, PatternPath, RepeatOptions};

/// v12 `createValuePattern`, with the regexes' tokens set as there.
fn value_pattern() -> Pattern {
    let digits = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0"];
    let values = rep(
        "values",
        reference("value"),
        RepeatOptions {
            divider: Some(re_tokens("divider", r"\s*,\s+", &[", "])),
            ..RepeatOptions::default()
        },
    );
    let array = seq(
        "array",
        vec![lit("open-bracket", "["), values, lit("close-bracket", "]")],
    );
    choice("value", vec![re_tokens("number", r"\d+", &digits), array])
}

/// Where v12's `findPattern(value, p => p.type === "reference")` lands.
fn the_reference(value: &Pattern) -> PatternPath {
    value
        .find(|p| p.type_name() == "reference")
        .expect("value has a reference")
        .path
}

// ─── Reference.test.ts ───

#[test]
fn reference_get_tokens() {
    // Reference.test.ts 'Get Tokens'
    let value = value_pattern();
    let reference = the_reference(&value);

    assert_eq!(reference, at(&[1, 1, 0]));
    assert_eq!(
        value.introspect().tokens(&reference),
        ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "["]
    );
}

#[test]
fn reference_get_tokens_after_with_no_parent() {
    // Reference.test.ts 'Get Tokens After With No Parent'
    assert!(reference("bad-reference").tokens_after(0).is_empty());
}

#[test]
fn reference_properties() {
    // Reference.test.ts 'Properties': a reference that names nothing has no
    // child to follow.
    let reference = reference("ref");

    assert_eq!(reference.type_name(), "reference");
    assert_eq!(reference.name(), "ref");
    assert!(reference.introspect().at(&at(&[0])).is_none());
}

#[test]
fn reference_find_pattern() {
    // Reference.test.ts 'Find Pattern': a reference's own `find` answers
    // nothing; what it names is found by name, not contained.
    assert!(reference("value").find(|p| p.name() == "Nada").is_none());
    assert!(reference("value").find(|p| p.name() == "value").is_none());
}

#[test]
fn reference_get_next_tokens() {
    // Reference.test.ts 'Get Next Tokens'
    let value = value_pattern();
    let reference = the_reference(&value);

    assert_eq!(value.next_tokens(&reference), [", ", "]"]);
}

#[test]
fn reference_get_next_tokens_with_null_parent() {
    // Reference.test.ts 'Get Next Tokens With Null Parent'
    assert!(
        reference("ref-name")
            .next_tokens(&PatternPath::root())
            .is_empty()
    );
}

#[test]
fn reference_get_tokens_after() {
    // Reference.test.ts 'Get Tokens After': whatever the child, what follows
    // the reference.
    let value = value_pattern();
    let reference = the_reference(&value);

    assert_eq!(value.introspect().tokens_after(&reference, 0), [", ", "]"]);
    assert_eq!(value.introspect().tokens_after(&reference, 9), [", ", "]"]);
}

#[test]
fn reference_get_tokens_after_with_null_parent() {
    // Reference.test.ts 'Get Tokens After With Null Parent'
    assert!(reference("ref-name").tokens_after(0).is_empty());
}

#[test]
fn reference_get_patterns() {
    // Reference.test.ts 'Get Patterns'
    let value = value_pattern();
    let reference = the_reference(&value);
    let patterns = value.introspect().patterns(&reference);

    assert_eq!(names(&patterns), ["number", "open-bracket"]);
    // Inside the resolved `value`: its atoms, then the array's first child.
    assert_eq!(paths(&patterns), ["/1/1/0/0/0", "/1/1/0/0/1/0"]);
}

#[test]
fn reference_get_patterns_after() {
    // Reference.test.ts 'Get Patterns After'
    let value = value_pattern();
    let reference = the_reference(&value);
    let patterns = value.introspect().patterns_after(&reference, 0);

    assert_eq!(names(&patterns), ["divider", "close-bracket"]);
    assert_eq!(patterns[0].pattern.type_name(), "regex");
    assert_eq!(patterns[1].pattern.type_name(), "literal");
}

#[test]
fn reference_get_patterns_after_with_null_parent() {
    // Reference.test.ts 'Get Patterns After With Null Parent'
    assert!(reference("ref-name").patterns_after(0).is_empty());
}

#[test]
fn reference_get_next_patterns() {
    // Reference.test.ts 'Get Next Patterns'
    let value = value_pattern();
    let reference = the_reference(&value);
    let patterns = value.next_patterns(&reference);

    assert_eq!(names(&patterns), ["divider", "close-bracket"]);
    assert_eq!(paths(&patterns), ["/1/1/1", "/1/2"]);
}

#[test]
fn reference_get_next_patterns_with_null_parent() {
    // Reference.test.ts 'Get Next Patterns With Null Parent'
    assert!(
        reference("ref-name")
            .next_patterns(&PatternPath::root())
            .is_empty()
    );
}

#[test]
fn a_parsed_root_gives_the_same_answers_through_its_resolved_instances() {
    // After a parse, references hold their instances; introspection follows
    // them (they are what `at_path` replays) and answers as before.
    let mut value = value_pattern();
    assert!(value.test("[1, [2, 3]]"));
    let reference = the_reference(&value);
    let intro = value.introspect();

    assert_eq!(
        intro.tokens(&reference).last().map(String::as_str),
        Some("[")
    );
    assert_eq!(intro.next_tokens(&reference), [", ", "]"]);
    let deep = at(&[1, 1, 0, 0, 1, 1, 0]);
    assert_eq!(intro.at(&deep).map(|h| h.name()), Some("value"));
    assert_eq!(intro.next_tokens(&deep), [", ", "]"]);
}

#[test]
fn a_target_is_named_after_the_reference() {
    // v12's resolved instance is a clone renamed after the reference, and a
    // context passes that name to its child.
    let root = context(
        "defs",
        seq(
            "s",
            vec![reference_to("alias", "x"), reference_to("other", "k")],
        ),
        vec![lit("x", "X"), context("k", lit("inner", "I"), vec![])],
    );
    let intro = root.introspect();

    assert_eq!(intro.at(&at(&[0, 0, 0])).unwrap().name(), "alias");
    assert_eq!(names(&root.patterns()), ["alias"]);
    let k_child = intro.at(&at(&[0, 1, 0, 0])).unwrap();
    assert_eq!(k_child.name(), "other");
    assert_eq!(k_child.pattern.name(), "inner", "the definition itself");
}

#[test]
fn a_name_that_resolves_to_nothing_contributes_nothing() {
    // v12 throws "Couldn't find …"; a query answers without it, and
    // `check` (or `exec`) reports the name.
    let root = seq("s", vec![reference("missing"), lit("b", "B")]);

    assert!(root.tokens().is_empty());
    assert_eq!(root.next_tokens(&at(&[0])), ["B"]);
    assert!(root.check().is_err());
}

#[test]
fn an_implicit_target_resolves_its_names_at_home() {
    // DESIGN §11: `t` is defined inside context `k`, which defines `u`, and
    // is reached by a reference outside `k`. The parse resolves `t`'s `u`
    // to `k`'s, and so does introspection. (v12 throws "Couldn't find 'u'"
    // when the reference runs before the definition; the documented
    // deviation.)
    let mut root = seq(
        "root",
        vec![
            reference("t"),
            context(
                "k",
                seq(
                    "k-body",
                    vec![seq("t", vec![lit("a", "a"), reference("u")])],
                ),
                vec![lit("u", "U")],
            ),
        ],
    );
    let intro = root.introspect();

    assert_eq!(intro.next_tokens(&at(&[0, 0, 0])), ["U"]);
    assert_eq!(
        intro.at(&at(&[0, 0, 1, 0])).unwrap().pattern.type_name(),
        "literal"
    );
    assert!(root.test("aUaU"));
}

#[test]
fn a_context_found_by_its_own_name_offers_its_child() {
    // `k = "(" k? ")"`: the inner `k` is the context itself (v12
    // `getPatternWithinContext`). Checked against v12.
    let root = context(
        "k",
        seq(
            "list",
            vec![
                lit("open", "("),
                opt("more", reference("k")),
                lit("close", ")"),
            ],
        ),
        vec![],
    );
    let intro = root.introspect();

    assert_eq!(intro.next_tokens(&at(&[0, 0])), ["(", ")"]);
    let inner_open = at(&[0, 1, 0, 0, 0, 0]);
    assert_eq!(intro.at(&inner_open).unwrap().name(), "open");
    assert_eq!(intro.next_tokens(&inner_open), ["(", ")"]);
    assert_eq!(intro.next_tokens(&at(&[0, 1, 0, 0, 0, 2])), [")"]);
}

// ─── Context.test.ts ───

#[test]
fn context_is_equal_compares_structure() {
    // Context.test.ts 'isEqual compares two contexts with same structure'
    let a = context("ctx", lit("x", "x"), vec![]);
    let b = context("ctx", lit("x", "x"), vec![]);
    let c = context("ctx", lit("y", "y"), vec![]);

    assert!(a.is_equal(&b));
    assert!(!a.is_equal(&c));
}

#[test]
fn context_find_locates_a_pattern_within_the_child() {
    // Context.test.ts 'find locates a pattern within the context's child'
    let root = context(
        "ctx",
        seq("names", vec![lit("john", "John"), lit("jane", "Jane")]),
        vec![],
    );
    let found = root.find(|p| p.name() == "jane").unwrap();

    assert_eq!(found.name(), "jane");
    assert_eq!(found.path, at(&[0, 1]));
    assert!(root.find(|p| p.name() == "bob").is_none());
}

#[test]
fn context_find_is_its_childs_own_find() {
    // v12 `Context.find` is `this._pattern.find`: a literal child's `find`
    // answers nothing, not even the literal.
    let root = context("ctx", lit("x", "x"), vec![]);
    assert!(root.find(|p| p.name() == "x").is_none());
    assert!(root.find(|p| p.name() == "ctx").is_none());
}

#[test]
fn context_standalone_introspection_is_empty() {
    // Context.test.ts 'standalone introspection methods return empty (no
    // parent)'
    let root = context("ctx", lit("x", "x"), vec![]);

    assert!(root.tokens_after(0).is_empty());
    assert!(root.next_tokens(&PatternPath::root()).is_empty());
    assert!(root.patterns_after(0).is_empty());
    assert!(root.next_patterns(&PatternPath::root()).is_empty());
}

#[test]
fn context_introspection_delegates_to_the_parent_in_a_sequence() {
    // Context.test.ts 'introspection delegates to parent when embedded in
    // sequence'
    let statement = seq(
        "stmt",
        vec![
            context("ctx", re("word", "[a-z]+"), vec![]),
            lit("semi", ";"),
        ],
    );
    let intro = statement.introspect();

    assert_eq!(intro.next_tokens(&at(&[0])), [";"]);
    assert_eq!(intro.next_patterns(&at(&[0])).len(), 1);
    assert_eq!(intro.tokens_after(&at(&[0]), 0), [";"]);
    assert_eq!(intro.patterns_after(&at(&[0]), 0).len(), 1);
}

#[test]
fn context_get_tokens_delegates_to_the_child() {
    // Context.test.ts 'getTokens delegates to inner pattern'
    let root = context("ctx", lit("hello", "hello"), vec![]);
    assert_eq!(root.tokens(), ["hello"]);
}

#[test]
fn context_get_patterns_delegates_to_the_child() {
    // Context.test.ts 'getPatterns delegates to inner pattern'
    let root = context("ctx", lit("x", "x"), vec![]);
    assert_eq!(names(&root.patterns()), ["x"]);
}

#[test]
fn a_reference_inside_a_context_resolves_to_its_definitions() {
    // Context.test.ts 'Basic', introspected: `names = john space jane` with
    // `space` defined by the context. Checked against v12.
    let statement = seq(
        "stmt",
        vec![
            context(
                "context",
                seq(
                    "names",
                    vec![lit("john", "John"), reference("space"), lit("jane", "Jane")],
                ),
                vec![lit("space", " ")],
            ),
            lit("semi", ";"),
        ],
    );

    assert_eq!(statement.next_tokens(&at(&[0, 0, 0])), [" "]);
    assert_eq!(statement.next_tokens(&at(&[0, 0, 1, 0])), ["Jane"]);
    assert_eq!(statement.next_tokens(&at(&[0, 0, 2])), [";"]);
}

#[test]
fn after_a_parse_an_implicit_target_still_resolves_at_home() {
    // The same grammar, parsed first: `rt`'s instance keeps the home `exec`
    // found for it in the scope `exec` readied, and introspection uses that
    // scope, so the home still applies (`scope::home_applies` compares
    // scopes by identity).
    let mut root = seq(
        "root",
        vec![
            reference("t"),
            context(
                "k",
                seq(
                    "k-body",
                    vec![seq("t", vec![lit("a", "a"), reference("u")])],
                ),
                vec![lit("u", "U")],
            ),
        ],
    );
    assert!(root.test("aUaU"));
    let intro = root.introspect();

    assert_eq!(intro.next_tokens(&at(&[0, 0, 0])), ["U"]);
    assert_eq!(intro.tokens(&at(&[0, 0, 1])), ["U"]);
}

#[test]
fn a_context_reached_under_another_name_answers_to_that_name() {
    // `alias` names context `k`; inside that instance the context answers to
    // `alias` too (a renamed context's frame), so `x -> alias` resolves
    // there. Checked against v12: `["(", "(", ")"]`.
    let root = context(
        "k",
        seq(
            "list",
            vec![
                lit("open", "("),
                opt("more", reference_to("alias", "k")),
                opt("again", reference_to("x", "alias")),
                lit("close", ")"),
            ],
        ),
        vec![],
    );
    assert_eq!(root.next_tokens(&at(&[0, 1, 0, 0, 0, 0])), ["(", "(", ")"]);
}

/// `root = t ("!" | t) k`, where `t = "a" ("b" | u)` is defined inside
/// context `k`, which defines `u`. Parsing `ab!ab` reaches neither the
/// second `t` nor any `u`.
fn parsed_with_unreached_references() -> Pattern {
    let t = seq(
        "t",
        vec![
            lit("a", "a"),
            choice("c", vec![lit("b", "b"), reference("u")]),
        ],
    );
    let mut root = seq(
        "root",
        vec![
            reference("t"),
            choice("c2", vec![lit("bang", "!"), reference("t")]),
            context("k", seq("k-body", vec![t]), vec![lit("u", "U")]),
        ],
    );
    assert!(root.test("ab!ab"));
    root
}

#[test]
fn a_parsed_instance_resolves_its_unparsed_names_at_home() {
    // The first `t` parsed, so it has an instance with the home `exec`
    // found; its `u` never parsed, so the query looks it up, under that
    // home (which belongs to the scope `exec` readied).
    let root = parsed_with_unreached_references();
    assert_eq!(root.next_tokens(&at(&[0, 0, 0])), ["b", "U"]);
}

#[test]
fn an_unparsed_reference_in_a_parsed_root_finds_its_target_at_home() {
    // The second `t` never parsed: it is looked up in the scope `exec`
    // readied, whose entry for `t` carries its home.
    let root = parsed_with_unreached_references();
    assert_eq!(root.next_tokens(&at(&[1, 1, 0, 0])), ["b", "U"]);
}

#[test]
fn a_parsed_reference_hands_out_the_instance_at_path_replays() {
    let root = parsed_with_unreached_references();
    let path = at(&[0, 0, 0]);
    let handle = root.introspect().at(&path).unwrap();

    assert!(std::ptr::eq(handle.pattern, root.at_path(&path).unwrap()));
}
