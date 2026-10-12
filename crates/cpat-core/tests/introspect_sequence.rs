//! Introspection of sequences, optionals and lookaheads, ported from v12
//! `Sequence.test.ts`, `Optional.test.ts` and `Not.test.ts` (each test
//! names its source). A v12 test that gives the pattern a parent asks about
//! a path inside a root; "With No Parent" asks about the root (empty path).
//! v12 passes a child pattern to `getTokensAfter`; here it is the child's
//! index, and a "bogus" child is an index that names no child.

mod common;

use common::*;
use cpat_core::PatternPath;

// ─── Sequence.test.ts ───

#[test]
fn sequence_properties() {
    // Sequence.test.ts 'Properties' and 'Set Parent'
    let sequence = seq("sequence", vec![lit("a", "A")]);
    let child = sequence.introspect().at(&at(&[0])).unwrap();

    assert_eq!(sequence.type_name(), "sequence");
    assert_eq!(sequence.name(), "sequence");
    assert_eq!(child.pattern.type_name(), "literal");
    assert_eq!(child.name(), "a");
}

#[test]
fn sequence_get_tokens() {
    // Sequence.test.ts 'Get Tokens': an optional first child lets the next
    // one start the sequence too.
    let sequence = seq("sequence", vec![opt("a", lit("a", "A")), lit("b", "B")]);
    assert_eq!(sequence.tokens(), ["A", "B"]);
}

#[test]
fn sequence_get_tokens_after() {
    // Sequence.test.ts 'Get Tokens After'
    let sequence = seq(
        "sequence",
        vec![lit("a", "A"), opt("b", lit("b", "B")), lit("c", "C")],
    );
    assert_eq!(sequence.tokens_after(0), ["B", "C"]);
}

#[test]
fn sequence_get_tokens_after_with_invalid_pattern() {
    // Sequence.test.ts 'Get Tokens After With Invalid Pattern'
    let sequence = seq(
        "sequence",
        vec![lit("a", "A"), opt("b", lit("b", "B")), lit("c", "C")],
    );
    assert!(sequence.tokens_after(3).is_empty());
}

#[test]
fn sequence_get_tokens_after_with_last_child() {
    // Sequence.test.ts 'Get Tokens After With Last Child': past its last
    // child, a sequence offers what follows it in its parent.
    let parent = seq(
        "parent",
        vec![seq("sequence", vec![lit("a", "A")]), lit("b", "B")],
    );
    assert_eq!(parent.introspect().tokens_after(&at(&[0]), 0), ["B"]);
}

#[test]
fn sequence_get_tokens_after_with_last_optional_child() {
    // Sequence.test.ts 'Get Tokens After With Last Optional Child'
    let parent = seq(
        "parent",
        vec![
            seq("sequence", vec![lit("a", "A"), opt("b", lit("b", "B"))]),
            lit("c", "C"),
        ],
    );
    assert_eq!(parent.introspect().tokens_after(&at(&[0]), 0), ["B", "C"]);
}

#[test]
fn sequence_get_next_tokens() {
    // Sequence.test.ts 'Get Next Tokens'
    let parent = seq(
        "parent",
        vec![seq("sequence", vec![lit("a", "A")]), lit("b", "B")],
    );
    let sequence = path_of(&parent, "sequence");

    assert_eq!(parent.next_tokens(&sequence), ["B"]);
}

#[test]
fn sequence_get_next_tokens_with_null_parent() {
    // Sequence.test.ts 'Get Next Tokens With Null Parent'
    let sequence = seq("sequence", vec![lit("a", "A")]);
    assert!(sequence.next_tokens(&PatternPath::root()).is_empty());
}

#[test]
fn sequence_get_patterns() {
    // Sequence.test.ts 'Get Patterns'
    let sequence = seq("sequence", vec![opt("a", lit("a", "A")), lit("b", "B")]);
    let patterns = sequence.patterns();

    // The optional named `a` is no leaf; its literal `a` is.
    assert_eq!(names(&patterns), ["a", "b"]);
    assert_eq!(paths(&patterns), ["/0/0", "/1"]);
    assert_eq!(patterns[0].pattern.type_name(), "literal");
}

#[test]
fn sequence_get_next_patterns() {
    // Sequence.test.ts 'Get Next Patterns'
    let parent = seq(
        "parent",
        vec![seq("sequence", vec![lit("a", "A")]), lit("b", "B")],
    );
    let sequence = path_of(&parent, "sequence");
    let b = parent.find(|p| p.name() == "b").unwrap();

    let next = parent.next_patterns(&sequence);

    assert!(next[0].pattern.is_equal(b.pattern));
    assert_eq!(next[0].path, b.path);
}

#[test]
fn sequence_get_next_patterns_with_null_parent() {
    // Sequence.test.ts 'Get Next Patterns With Null Parent'
    let sequence = seq("sequence", vec![lit("a", "A")]);
    assert!(sequence.next_patterns(&PatternPath::root()).is_empty());
}

#[test]
fn a_lookahead_leading_a_sequence_offers_what_follows_it_then_the_next_child() {
    // Checked against v12: `!x` contributes its next tokens (`Not.getTokens`
    // is `getNextTokens`), and the sequence carries on past a lookahead, so
    // the token after it is offered twice.
    let sequence = seq(
        "sequence",
        vec![not("n", lit("x", "X")), lit("b", "B"), lit("c", "C")],
    );
    assert_eq!(sequence.tokens(), ["B", "B"]);
}

#[test]
fn a_recursive_first_child_offers_nothing() {
    // v12 `Sequence.getTokens` stops at a first child already being
    // expanded twice above it (`isRecursivePattern`). `r = r2 "z"`,
    // `r2 = (r "y") | "w"`: expanding `r` reaches `r` again through `r2`,
    // first in every sequence on the way. The guard needs two ancestors
    // with the child's id, so one inner level is expanded too, and `w` is
    // offered by both levels. Checked against v12: `["w", "w"]`.
    let root = context(
        "defs",
        reference("r"),
        vec![
            seq("r", vec![reference("r2"), lit("z", "z")]),
            choice(
                "r2",
                vec![
                    seq("r3", vec![reference("r"), lit("y", "y")]),
                    lit("w", "w"),
                ],
            ),
        ],
    );
    assert_eq!(root.tokens(), ["w", "w"]);
}

mod sequence_is_equal {
    // Sequence.test.ts 'isEqual'
    use super::*;

    fn three_children() -> cpat_core::Pattern {
        seq(
            "sequence",
            vec![lit("a", "A"), lit("b", "B"), lit("c", "C")],
        )
    }

    #[test]
    fn matches_an_identical_sequence() {
        assert!(three_children().is_equal(&three_children()));
    }

    #[test]
    fn does_not_match_when_a_child_differs() {
        let other = seq(
            "sequence",
            vec![lit("a", "A"), lit("b", "B"), lit("c", "DIFFERENT")],
        );
        assert!(!three_children().is_equal(&other));
    }

    #[test]
    fn does_not_match_when_the_other_sequence_has_fewer_children() {
        let fewer = seq("sequence", vec![lit("a", "A"), lit("b", "B")]);
        assert!(!three_children().is_equal(&fewer));
    }

    #[test]
    fn does_not_match_when_the_other_sequence_has_more_children() {
        let fewer = seq("sequence", vec![lit("a", "A"), lit("b", "B")]);
        assert!(!fewer.is_equal(&three_children()));
    }

    #[test]
    fn does_not_match_a_different_pattern_type() {
        assert!(!three_children().is_equal(&lit("a", "A")));
    }

    #[test]
    fn names_do_not_count() {
        // v12 compares types and children only.
        let renamed = seq("other", vec![lit("x", "A"), lit("y", "B"), lit("z", "C")]);
        assert!(three_children().is_equal(&renamed));
    }
}

// ─── Optional.test.ts ───

fn about_us() -> cpat_core::Pattern {
    opt("optional-about-us", lit("about-us", "About Us"))
}

fn about_sequence() -> cpat_core::Pattern {
    seq(
        "sequence",
        vec![about_us(), lit("about-them", "About Them")],
    )
}

#[test]
fn optional_clone() {
    // Optional.test.ts 'Clone'
    let optional = opt("optional-a", lit("a", "A"));
    let clone = optional.clone_named("optional-a");

    assert_eq!(clone.name(), "optional-a");
    assert_eq!(clone.id(), optional.id());
}

#[test]
fn optional_tokens() {
    // Optional.test.ts 'Tokens'
    let optional = opt("optional-a", lit("a", "A"));

    assert_eq!(optional.tokens(), ["A"]);
    assert!(optional.tokens_after(5).is_empty());
}

#[test]
fn optional_properties() {
    // Optional.test.ts 'Properties'
    let optional = opt("optional-a", lit("a", "A"));

    assert_eq!(optional.type_name(), "optional");
    assert_eq!(optional.name(), "optional-a");
    assert_eq!(optional.introspect().at(&at(&[0])).unwrap().name(), "a");
}

#[test]
fn optional_get_next_tokens() {
    // Optional.test.ts 'Get Next Tokens'
    let sequence = about_sequence();
    let optional = path_of(&sequence, "optional-about-us");

    assert_eq!(sequence.next_tokens(&optional), ["About Them"]);
}

#[test]
fn optional_get_next_tokens_with_no_parent() {
    // Optional.test.ts 'Get Next Tokens With No Parent'
    assert!(about_us().next_tokens(&PatternPath::root()).is_empty());
}

#[test]
fn optional_get_tokens() {
    // Optional.test.ts 'Get Tokens'
    let sequence = about_sequence();
    let optional = path_of(&sequence, "optional-about-us");

    assert_eq!(sequence.introspect().tokens(&optional), ["About Us"]);
}

#[test]
fn optional_get_tokens_after() {
    // Optional.test.ts 'Get Tokens After': after its child, what follows
    // the optional.
    let sequence = about_sequence();
    let optional = path_of(&sequence, "optional-about-us");

    assert_eq!(
        sequence.introspect().tokens_after(&optional, 0),
        ["About Them"]
    );
}

#[test]
fn optional_find_pattern() {
    // Optional.test.ts 'Find Pattern'
    let found = about_us().find(|p| p.name() == "about-us").map(|h| h.path);
    assert_eq!(found, Some(at(&[0])));
}

#[test]
fn optional_get_patterns() {
    // Optional.test.ts 'Get Patterns'
    let sequence = about_sequence();
    let optional = path_of(&sequence, "optional-about-us");
    let about_us = sequence.find(|p| p.name() == "about-us").unwrap();

    let patterns = sequence.introspect().patterns(&optional);

    assert_eq!(patterns.len(), 1);
    assert!(std::ptr::eq(patterns[0].pattern, about_us.pattern));
    assert_eq!(patterns[0].path, about_us.path);
}

#[test]
fn optional_get_next_patterns() {
    // Optional.test.ts 'Get Next Patterns'
    let sequence = about_sequence();
    let optional = path_of(&sequence, "optional-about-us");

    assert_eq!(names(&sequence.next_patterns(&optional)), ["about-them"]);
}

#[test]
fn optional_get_next_patterns_with_no_parent() {
    // Optional.test.ts 'Get Next Patterns With No Parent'
    assert!(about_us().next_patterns(&PatternPath::root()).is_empty());
}

#[test]
fn optional_get_patterns_after() {
    // Optional.test.ts 'Get Patterns After'
    let sequence = about_sequence();
    let optional = path_of(&sequence, "optional-about-us");

    let patterns = sequence.introspect().patterns_after(&optional, 0);

    assert_eq!(names(&patterns), ["about-them"]);
}

#[test]
fn optional_get_patterns_after_with_null_parent() {
    // Optional.test.ts 'Get Patterns After With Null Parent'
    assert!(about_us().patterns_after(0).is_empty());
}

// ─── Not.test.ts ───

fn not_about_us() -> cpat_core::Pattern {
    not("not-about-us", lit("about-us", "About Us"))
}

fn not_sequence() -> cpat_core::Pattern {
    seq(
        "sequence",
        vec![not_about_us(), lit("about-them", "About Them")],
    )
}

#[test]
fn not_clone() {
    // Not.test.ts 'Clone'
    let not_a = not("not-a", lit("a", "A"));
    let clone = not_a.clone_named("not-a");

    assert_eq!(clone.name(), "not-a");
    assert_eq!(clone.id(), not_a.id());
}

#[test]
fn not_tokens() {
    // Not.test.ts 'Tokens': with no parent nothing follows it, so it offers
    // nothing.
    let not_a = not("not-a", lit("a", "A"));

    assert!(not_a.tokens().is_empty());
    assert!(not_a.tokens_after(5).is_empty());
}

#[test]
fn not_properties() {
    // Not.test.ts 'Properties'
    let not_a = not("not-a", lit("a", "A"));

    assert_eq!(not_a.type_name(), "not");
    assert_eq!(not_a.name(), "not-a");
    assert_eq!(not_a.introspect().at(&at(&[0])).unwrap().name(), "a");
}

#[test]
fn not_get_next_tokens() {
    // Not.test.ts 'Get Next Tokens'
    let sequence = not_sequence();
    let not_path = path_of(&sequence, "not-about-us");

    assert_eq!(sequence.next_tokens(&not_path), ["About Them"]);
}

#[test]
fn not_get_next_tokens_with_no_parent() {
    // Not.test.ts 'Get Next Tokens With No Parent'
    assert!(not_about_us().next_tokens(&PatternPath::root()).is_empty());
}

#[test]
fn not_get_tokens() {
    // Not.test.ts 'Get Tokens': a lookahead matches nothing, so it offers
    // what follows it.
    let sequence = not_sequence();
    let not_path = path_of(&sequence, "not-about-us");

    assert_eq!(sequence.introspect().tokens(&not_path), ["About Them"]);
}

#[test]
fn not_get_tokens_after() {
    // Not.test.ts 'Get Tokens After'
    let sequence = not_sequence();
    let not_path = path_of(&sequence, "not-about-us");

    assert_eq!(
        sequence.introspect().tokens_after(&not_path, 0),
        ["About Them"]
    );
}

#[test]
fn not_find_pattern() {
    // Not.test.ts 'Find Pattern'
    let found = not_about_us()
        .find(|p| p.name() == "about-us")
        .map(|h| h.path);
    assert_eq!(found, Some(at(&[0])));
}

#[test]
fn not_get_patterns() {
    // Not.test.ts 'Get Patterns': what follows it, not its child.
    let sequence = not_sequence();
    let not_path = path_of(&sequence, "not-about-us");
    let about_them = sequence.find(|p| p.name() == "about-them").unwrap();

    let patterns = sequence.introspect().patterns(&not_path);

    assert_eq!(patterns.len(), 1);
    assert!(std::ptr::eq(patterns[0].pattern, about_them.pattern));
}

#[test]
fn not_get_next_patterns() {
    // Not.test.ts 'Get Next Patterns'
    let sequence = not_sequence();
    let not_path = path_of(&sequence, "not-about-us");

    assert_eq!(names(&sequence.next_patterns(&not_path)), ["about-them"]);
}

#[test]
fn not_get_next_patterns_with_no_parent() {
    // Not.test.ts 'Get Next Patterns With No Parent'
    assert!(
        not_about_us()
            .next_patterns(&PatternPath::root())
            .is_empty()
    );
}

#[test]
fn not_get_patterns_after() {
    // Not.test.ts 'Get Patterns After'
    let sequence = not_sequence();
    let not_path = path_of(&sequence, "not-about-us");

    assert_eq!(
        names(&sequence.introspect().patterns_after(&not_path, 0)),
        ["about-them"]
    );
}

#[test]
fn not_get_patterns_after_with_null_parent() {
    // Not.test.ts 'Get Patterns After With Null Parent'
    assert!(not_about_us().patterns_after(0).is_empty());
}

#[test]
fn an_expansion_that_never_ends_stops_at_the_depth_limit() {
    // `a = "x"? a`: the optional lets the sequence reach its second child,
    // which is `a` again, and only a FIRST child is checked for recursion.
    // v12 recurses until the JS engine throws `RangeError`. Here the chain
    // stops growing at a fixed depth (DESIGN §16), so the query returns,
    // without overflowing the test thread's stack, with the `x` each level
    // offered before it.
    let root = context(
        "defs",
        reference("a"),
        vec![seq("a", vec![opt("ox", lit("x", "x")), reference("a")])],
    );
    let intro = root.introspect();

    let tokens = intro.tokens(&PatternPath::root());
    assert!(!tokens.is_empty() && tokens.len() < 256, "{}", tokens.len());
    assert!(tokens.iter().all(|t| t == "x"));
    let patterns = intro.patterns(&PatternPath::root());
    assert_eq!(patterns.len(), tokens.len());
    // Asking from deep inside works the same way.
    let deep = intro.at(&patterns[patterns.len() - 1].path).unwrap();
    assert_eq!(deep.name(), "x");
}

#[test]
fn an_expansion_that_never_ends_and_branches_stops_at_the_work_budget() {
    // `a = "x"? ("y"? a)? a`: every level offers two ways back into `a`, so
    // the depth limit alone would leave 2^256 places to make. Each query
    // makes at most a fixed number of places (DESIGN §16), then answers with
    // what it found. (v12 throws `RangeError`.)
    let root = context(
        "defs",
        reference("a"),
        vec![seq(
            "a",
            vec![
                opt("ox", lit("x", "x")),
                opt("oy", choice("c", vec![reference("a"), lit("y", "y")])),
                reference("a"),
            ],
        )],
    );
    let intro = root.introspect();

    let tokens = intro.tokens(&PatternPath::root());
    assert!(
        !tokens.is_empty() && tokens.len() < 100_000,
        "{}",
        tokens.len()
    );
    // The budget is per query: the next one gets all of it again.
    assert_eq!(intro.tokens(&PatternPath::root()), tokens);
}

#[test]
fn a_lookahead_offers_the_leaves_of_what_follows_it() {
    // v12 `Not.getPatterns` is `getNextPatterns().flatMap(getPatterns)`:
    // the following sequence's first leaf, not the sequence. Checked against
    // v12.
    let root = seq(
        "s",
        vec![
            not("n", lit("x", "X")),
            seq("inner", vec![lit("a", "A"), lit("b", "B")]),
        ],
    );
    let patterns = root.introspect().patterns(&at(&[0]));

    assert_eq!(names(&patterns), ["a"]);
    assert_eq!(paths(&patterns), ["/1/0"]);
}

#[test]
fn find_searches_children_before_their_parent() {
    // v12 `findPattern`: a child with the parent's name is found first.
    let root = seq("a", vec![lit("a", "A")]);
    assert_eq!(root.find(|p| p.name() == "a").unwrap().path, at(&[0]));
}

#[test]
fn references_are_equal_by_name() {
    // v12 `Reference.isEqual` compares names.
    assert!(reference("a").is_equal(&reference("a")));
    assert!(!reference("a").is_equal(&reference("b")));
}
