//! Introspection of repeats, ported from v12 `Repeat.test.ts`,
//! `InfiniteRepeat.test.ts` and `FiniteRepeat.test.ts` (each test names its
//! source). An unbounded repeat's children are `[child, divider]`; a
//! bounded one's are v12's slots (child, divider, child, …), numbered as
//! `Pattern::at_path` numbers them.
//!
//! v12 wraps every repeat in a `Repeat` facade whose only child is the
//! `InfiniteRepeat`/`FiniteRepeat` that does the work. Rust has one
//! `Repeat`, and it answers as the inner repeat does with the facade as its
//! parent, which is what every query climbing from a child sees in v12.
//! Asking the facade itself `getTokensAfter(x)` ignores `x` and answers
//! what follows the facade; there is no such second pattern here (DESIGN
//! §16, deviations), so those three `Repeat.test.ts` assertions are pinned
//! with v12's facade answer in a comment.

mod common;

use common::*;
use cpat_core::{PatternPath, RepeatOptions};

fn bounded(
    min: usize,
    max: usize,
    divider: Option<cpat_core::Pattern>,
    trim: bool,
) -> RepeatOptions {
    RepeatOptions {
        min,
        max: Some(max),
        divider,
        trim_divider: trim,
    }
}

// ─── Repeat.test.ts ───

#[test]
fn repeat_get_tokens_finite_repeat_get_tokens() {
    // Repeat.test.ts 'Finite Repeat Get Tokens'
    let repeat = rep("numbers", lit("number", "1"), RepeatOptions::default());

    assert_eq!(repeat.tokens(), ["1"]);
    assert!(repeat.next_tokens(&PatternPath::root()).is_empty());
    // Deviation (no facade): v12's facade answers `[]` here, ignoring the
    // child. The repeat that owns the child offers the child again (and
    // nothing follows the root), which is v12's answer to the `next tokens`
    // of the child itself:
    assert_eq!(repeat.tokens_after(0), ["1"]);
    assert_eq!(repeat.next_tokens(&at(&[0])), ["1"]);
}

#[test]
fn repeat_get_tokens_after() {
    // Repeat.test.ts 'Get Tokens After'. Deviation (no facade): v12's facade
    // answers `["B"]`, what follows it; the repeat that owns `number`
    // offers `number` again or what follows: v12's `number.getNextTokens()`.
    let parent = seq(
        "parent",
        vec![
            rep("numbers", lit("number", "1"), RepeatOptions::default()),
            lit("b", "B"),
        ],
    );

    assert_eq!(parent.introspect().tokens_after(&at(&[0]), 0), ["1", "B"]);
    assert_eq!(parent.next_tokens(&path_of(&parent, "number")), ["1", "B"]);
}

#[test]
fn repeat_get_next_tokens() {
    // Repeat.test.ts 'Get Next Tokens'
    let parent = seq(
        "parent",
        vec![
            rep("numbers", lit("number", "1"), RepeatOptions::default()),
            lit("b", "B"),
        ],
    );
    assert_eq!(parent.next_tokens(&at(&[0])), ["B"]);
}

#[test]
fn repeat_get_next_patterns() {
    // Repeat.test.ts 'Get Next Patterns'
    let parent = seq(
        "parent",
        vec![
            rep("numbers", lit("number", "1"), RepeatOptions::default()),
            lit("b", "B"),
        ],
    );
    let b = parent.find(|p| p.name() == "b").unwrap();
    let next = parent.next_patterns(&at(&[0]));

    assert_eq!(next.len(), 1);
    assert!(std::ptr::eq(next[0].pattern, b.pattern));
}

#[test]
fn repeat_get_patterns() {
    // Repeat.test.ts 'Repeat Get Patterns'
    let repeat = rep("numbers", lit("number", "1"), RepeatOptions::default());
    let number = repeat.find(|p| p.name() == "number").unwrap();

    let patterns = repeat.patterns();
    assert_eq!(patterns.len(), 1);
    assert!(std::ptr::eq(patterns[0].pattern, number.pattern));
    assert_eq!(patterns[0].path, number.path);

    assert!(repeat.next_patterns(&PatternPath::root()).is_empty());
    // Deviation (no facade): v12's facade answers `[]`; the repeat that
    // owns `number` offers `number` again.
    assert_eq!(names(&repeat.patterns_after(0)), ["number"]);
}

#[test]
fn repeat_properties() {
    // Repeat.test.ts 'Repeat Properties'
    let repeat = rep("numbers", lit("number", "1"), RepeatOptions::default());

    assert_eq!(repeat.type_name(), "infinite-repeat");
    assert_eq!(repeat.name(), "numbers");
    let bounded = rep("numbers", lit("number", "1"), bounded(1, 2, None, false));
    assert_eq!(bounded.type_name(), "finite-repeat");
}

#[test]
fn repeat_clone() {
    // Repeat.test.ts 'Repeat Clone'
    let repeat = rep("numbers", lit("number", "1"), RepeatOptions::default());

    let clone = repeat.clone_named("numbers");
    assert!(clone.is_equal(&rep(
        "numbers",
        lit("number", "1"),
        RepeatOptions::default()
    )));

    let clone = repeat.clone_named("new-name");
    assert_eq!(clone.name(), "new-name");
    assert!(clone.is_equal(&rep(
        "new-name",
        lit("number", "1"),
        RepeatOptions::default()
    )));
}

// ─── InfiniteRepeat.test.ts ───

#[test]
fn infinite_get_tokens() {
    // InfiniteRepeat.test.ts 'Get Tokens'
    let many = rep("number", lit("a", "A"), RepeatOptions::default());
    assert_eq!(many.tokens(), ["A"]);
}

#[test]
fn infinite_get_tokens_after_with_bogus_pattern() {
    // InfiniteRepeat.test.ts 'Get Tokens After With Bogus Pattern'
    let many = rep("many-a", lit("a", "A"), RepeatOptions::default());
    assert!(many.tokens_after(2).is_empty());
    assert!(many.tokens_after(1).is_empty(), "no divider, no child 1");
}

#[test]
fn infinite_get_tokens_after_with_divider() {
    // InfiniteRepeat.test.ts 'Get Tokens After With Divider'
    let parent = seq(
        "parent",
        vec![
            rep("many-a", lit("a", "A"), divided(lit("divider", ","))),
            lit("b", "B"),
        ],
    );
    let many = path_of(&parent, "many-a");
    let intro = parent.introspect();

    // After the child: the divider, or the end of the repeat.
    assert_eq!(intro.tokens_after(&many, 0), [",", "B"]);
    // After the divider: the child.
    assert_eq!(intro.tokens_after(&many, 1), ["A"]);
}

#[test]
fn infinite_get_tokens_after_without_divider() {
    // InfiniteRepeat.test.ts 'Get Tokens After Without Divider'
    let parent = seq(
        "parent",
        vec![
            rep("many-a", lit("a", "A"), RepeatOptions::default()),
            lit("b", "B"),
        ],
    );
    let many = path_of(&parent, "many-a");

    assert_eq!(parent.introspect().tokens_after(&many, 0), ["A", "B"]);
}

#[test]
fn infinite_properties() {
    // InfiniteRepeat.test.ts 'Properties'
    let integer = rep("integer", re("digit", "\\d"), RepeatOptions::default());

    assert_eq!(integer.type_name(), "infinite-repeat");
    assert_eq!(integer.name(), "integer");
    assert_eq!(integer.introspect().at(&at(&[0])).unwrap().name(), "digit");
}

#[test]
fn infinite_get_next_tokens() {
    // InfiniteRepeat.test.ts 'Get Next Tokens'
    let parent = seq(
        "parent",
        vec![
            rep("integer", re("digit", "\\d"), RepeatOptions::default()),
            lit("pow", "!"),
        ],
    );
    let integer = path_of(&parent, "integer");

    assert_eq!(parent.next_tokens(&integer), ["!"]);
}

#[test]
fn infinite_get_next_tokens_with_null_parents() {
    // InfiniteRepeat.test.ts 'Get Next Tokens With Null Parents'
    let integer = rep("integer", re("digit", "\\d"), RepeatOptions::default());
    assert!(integer.next_tokens(&PatternPath::root()).is_empty());
}

#[test]
fn infinite_find_pattern() {
    // InfiniteRepeat.test.ts 'Find Pattern'
    let integer = rep("integer", re("digit", "\\d"), RepeatOptions::default());
    let digit = integer.find(|p| p.name() == "digit").unwrap();

    assert_eq!(digit.path, at(&[0]));
    // A repeat's own `find` searches itself too.
    assert_eq!(
        integer.find(|p| p.name() == "integer").unwrap().path,
        at(&[])
    );
}

#[test]
fn infinite_get_patterns() {
    // InfiniteRepeat.test.ts 'Get Patterns'
    let many = rep("number", lit("a", "A"), RepeatOptions::default());
    let a = many.find(|p| p.name() == "a").unwrap();
    let patterns = many.patterns();

    assert_eq!(patterns.len(), 1);
    assert!(std::ptr::eq(patterns[0].pattern, a.pattern));
}

#[test]
fn infinite_get_next_patterns() {
    // InfiniteRepeat.test.ts 'Get Next Patterns'
    let parent = seq(
        "parent",
        vec![
            rep("integer", re("digit", "\\d"), RepeatOptions::default()),
            lit("pow", "!"),
        ],
    );
    let integer = path_of(&parent, "integer");
    let pow = parent.find(|p| p.name() == "pow").unwrap();
    let next = parent.next_patterns(&integer);

    assert_eq!(next.len(), 1);
    assert!(std::ptr::eq(next[0].pattern, pow.pattern));
}

#[test]
fn infinite_get_next_patterns_with_null_parents() {
    // InfiniteRepeat.test.ts 'Get Next Patterns With Null Parents'
    let integer = rep("integer", re("digit", "\\d"), RepeatOptions::default());
    assert!(integer.next_patterns(&PatternPath::root()).is_empty());
}

#[test]
fn infinite_clone_with_custom_overrides() {
    // InfiniteRepeat.test.ts 'Clone With Custom Overrides'
    let numbers = || {
        rep(
            "numbers",
            re("number", "\\d"),
            RepeatOptions {
                min: 3,
                divider: Some(lit("divider", "divider")),
                trim_divider: true,
                ..RepeatOptions::default()
            },
        )
    };
    assert!(numbers().clone_named("numbers").is_equal(&numbers()));
}

#[test]
fn repeat_equality_compares_child_divider_and_bound() {
    // v12 compares type and children (a bounded repeat's slots).
    let plain = || rep("r", lit("a", "A"), RepeatOptions::default());
    let with_divider = rep("r", lit("a", "A"), divided(lit("d", ",")));
    let other_divider = rep("r", lit("a", "A"), divided(lit("d", ";")));
    let two = rep("r", lit("a", "A"), bounded(1, 2, None, false));
    let three = rep("r", lit("a", "A"), bounded(1, 3, None, false));
    let three_again = rep("x", lit("a", "A"), bounded(2, 3, None, false));

    assert!(!plain().is_equal(&with_divider));
    assert!(!with_divider.is_equal(&other_divider));
    assert!(!plain().is_equal(&three), "unbounded and bounded differ");
    assert!(!two.is_equal(&three), "a different slot count");
    assert!(three.is_equal(&three_again), "min isn't a child");
}

// ─── FiniteRepeat.test.ts ───

fn ones(max: usize, trim: bool) -> cpat_core::Pattern {
    rep(
        "numbers",
        lit("one", "1"),
        bounded(0, max, Some(lit("comma", ",")), trim),
    )
}

#[test]
fn finite_properties() {
    // FiniteRepeat.test.ts 'Properties': three slots, no divider.
    let numbers = rep("numbers", re("number", "\\d"), bounded(1, 3, None, false));
    let intro = numbers.introspect();

    assert_eq!(numbers.type_name(), "finite-repeat");
    assert_eq!(numbers.name(), "numbers");
    assert!(intro.at(&at(&[2])).is_some());
    assert!(intro.at(&at(&[3])).is_none());
}

#[test]
fn finite_clone() {
    // FiniteRepeat.test.ts 'Clone'
    let numbers = rep("numbers", re("number", "\\d"), bounded(0, 3, None, false));
    let clone = numbers.clone_named("numbers");

    assert_eq!(clone.type_name(), numbers.type_name());
    assert_eq!(clone.name(), numbers.name());
    assert_eq!(clone.id(), numbers.id());
    assert!(clone.introspect().at(&at(&[2])).is_some());
    assert!(clone.introspect().at(&at(&[3])).is_none());
}

#[test]
fn finite_clone_with_custom_overrides() {
    // FiniteRepeat.test.ts 'Clone With Custom Overrides'
    let numbers = |name: &str| rep(name, re("number", "\\d"), bounded(0, 3, None, false));

    assert!(
        numbers("numbers")
            .clone_named("numbers")
            .is_equal(&numbers("numbers"))
    );
    assert!(
        numbers("numbers")
            .clone_named("cloned-numbers")
            .is_equal(&numbers("cloned-numbers"))
    );
}

#[test]
fn finite_get_tokens() {
    // FiniteRepeat.test.ts 'Get Tokens'
    assert_eq!(ones(3, false).tokens(), ["1"]);
}

#[test]
fn finite_get_tokens_after_without_parent() {
    // FiniteRepeat.test.ts 'Get Tokens After Without Parent': slots
    // `[1, ",", 1]` (trimmed: no divider after the last child).
    let numbers = ones(2, true);

    assert_eq!(numbers.tokens_after(0), [","]);
    assert!(numbers.tokens_after(2).is_empty(), "the last slot");
    assert!(numbers.tokens_after(3).is_empty(), "no slot 3");
}

#[test]
fn finite_get_tokens_after_with_parent() {
    // FiniteRepeat.test.ts 'Get Tokens After With Parent'
    let parent = seq("parent", vec![ones(2, true), lit("b", "B")]);
    let intro = parent.introspect();

    assert_eq!(intro.tokens_after(&at(&[0]), 0), [","]);
    assert_eq!(intro.tokens_after(&at(&[0]), 1), ["1"]);
    assert_eq!(intro.tokens_after(&at(&[0]), 2), ["B"]);
    assert!(intro.tokens_after(&at(&[0]), 3).is_empty());
}

#[test]
fn finite_get_next_tokens() {
    // FiniteRepeat.test.ts 'Get Next Tokens'
    let parent = seq("parent", vec![ones(2, false), lit("b", "B")]);
    assert_eq!(parent.next_tokens(&at(&[0])), ["B"]);
}

#[test]
fn finite_get_next_tokens_without_parent() {
    // FiniteRepeat.test.ts 'Get Next Tokens Without Parent'
    assert!(ones(2, false).next_tokens(&PatternPath::root()).is_empty());
}

#[test]
fn finite_get_patterns() {
    // FiniteRepeat.test.ts 'Get Patterns': the first slot's.
    let numbers = ones(2, false);
    let patterns = numbers.patterns();

    assert_eq!(names(&patterns), ["one"]);
    assert_eq!(paths(&patterns), ["/0"]);
}

#[test]
fn finite_get_next_patterns_without_parent() {
    // FiniteRepeat.test.ts 'Get Next Patterns Without Parent'
    assert!(
        ones(2, false)
            .next_patterns(&PatternPath::root())
            .is_empty()
    );
}

#[test]
fn finite_get_next_patterns_with_parent() {
    // FiniteRepeat.test.ts 'Get Next Patterns With Parent'
    let parent = seq("parent", vec![ones(2, false), lit("b", "B")]);
    let next = parent.next_patterns(&at(&[0]));

    assert_eq!(paths(&next), ["/1"]);
    assert_eq!(names(&next), ["b"]);
}

#[test]
fn finite_find_pattern() {
    // FiniteRepeat.test.ts 'Find Pattern': the divider in slot 1.
    let numbers = ones(2, false);
    let comma = numbers.find(|p| p.name() == "comma").unwrap();

    assert_eq!(comma.path, at(&[1]));
}

#[test]
fn a_huge_bound_is_searched_and_walked_without_building_its_slots() {
    // A slot index is all a bounded repeat's child is (DESIGN §10), so a
    // bound of a million costs nothing to introspect.
    let numbers = ones(1_000_000, false);

    assert_eq!(
        numbers.find(|p| p.name() == "missing").map(|h| h.path),
        None
    );
    assert_eq!(numbers.tokens_after(1_999_998), [","]);
    assert!(numbers.tokens_after(1_999_999).is_empty(), "the last slot");
}

#[test]
fn a_bounded_repeat_offers_the_next_slots_leaves() {
    // v12 `FiniteRepeat.getPatternsAfter` returns `nextChild.getPatterns()`:
    // the divider's first leaf, not the divider. Checked against v12.
    let parent = seq(
        "p",
        vec![
            rep(
                "r",
                lit("one", "1"),
                bounded(
                    1,
                    2,
                    Some(seq("div", vec![lit("comma", ","), lit("sp", " ")])),
                    false,
                ),
            ),
            lit("end", "E"),
        ],
    );
    let after = parent.introspect().patterns_after(&at(&[0]), 0);

    assert_eq!(names(&after), ["comma"]);
    assert_eq!(paths(&after), ["/0/1/0"]);
}
