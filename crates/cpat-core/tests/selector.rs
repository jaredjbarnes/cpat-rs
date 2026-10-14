//! A port of clarity-pattern-parser v12's `src/query/selector.test.ts`, every
//! test, then what v12 does that its tests don't pin (each checked against v12
//! with the same selector and tree). `Selector::find(&tree, &[root])` is v12's
//! `new Selector(text).find([root])`.
//!
//! - Node Selector → `node_selector`
//! - Descendant Selector → `descendant_selector`
//! - Descendant Selector Miss → `descendant_selector_miss`
//! - Direct Child → `direct_child`
//! - More Than One Direct Child → `more_than_one_direct_child`
//! - Direct Child With Name & Attribute → `direct_child_with_name_and_attribute`
//! - Direct Child With Attribute → `direct_child_with_attribute`
//! - Adjacent → `adjacent`
//! - Adjacent Miss → `adjacent_miss`
//! - After → `after`
//! - After Miss → `after_miss`
//! - Not Equal Attribute → `not_equal_attribute`
//! - Starts With Attribute → `starts_with_attribute`
//! - Ends With Attribute → `ends_with_attribute`
//! - Contains Attribute → `contains_attribute`
//! - Greater Than Or Equal Attribute → `greater_than_or_equal_attribute`
//! - Less Than Or Equal Attribute → `less_than_or_equal_attribute`
//! - Greater Than Attribute → `greater_than_attribute`
//! - Less Than Attribute → `less_than_attribute`
//! - Or Selector → `or_selector`
//! - Nested Or Selector → `nested_or_selector`

mod common;

use common::{family_tree, joined};
use cpat_core::{EditableTree, NodeRef, Selector};

/// v12's `new Selector(selector).find([tree])` on the family tree: how many
/// nodes, and their values.
fn find(selector: &str) -> (usize, String) {
    let mut tree = EditableTree::new();
    let root = family_tree(&mut tree);
    let found = Selector::new(selector).unwrap().find(&tree, &[root]);
    (found.len(), joined(&tree, &found))
}

#[test]
fn node_selector() {
    assert_eq!(
        find("child"),
        (
            24,
            "1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24".to_owned()
        )
    );
}

#[test]
fn descendant_selector() {
    assert_eq!(
        find("grand-parent child"),
        (
            24,
            "1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24".to_owned()
        )
    );
}

#[test]
fn descendant_selector_miss() {
    assert_eq!(find("grand-parent not-a-node"), (0, "".to_owned()));
}

#[test]
fn direct_child() {
    assert_eq!(
        find("grand-parent > parent"),
        (6, "1234,5678,9101112,13141516,17181920,21222324".to_owned())
    );
}

#[test]
fn more_than_one_direct_child() {
    assert_eq!(
        find("grand-parent > parent > child"),
        (
            24,
            "1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24".to_owned()
        )
    );
}

#[test]
fn direct_child_with_name_and_attribute() {
    assert_eq!(
        find("grand-parent > parent > child[value='4']"),
        (1, "4".to_owned())
    );
}

#[test]
fn direct_child_with_attribute() {
    assert_eq!(
        find("grand-parent > parent > [value='4']"),
        (1, "4".to_owned())
    );
}

#[test]
fn adjacent() {
    assert_eq!(find("child + [value='4']"), (1, "4".to_owned()));
}

#[test]
fn adjacent_miss() {
    assert_eq!(find("parent + [value='4']"), (0, "".to_owned()));
}

#[test]
fn after() {
    assert_eq!(find("[value='2'] ~ *"), (2, "3,4".to_owned()));
}

#[test]
fn after_miss() {
    assert_eq!(find("[value='2'] ~ not-a-node"), (0, "".to_owned()));
}

#[test]
fn not_equal_attribute() {
    assert_eq!(find("[value!='4']"), (32, "1,2,3,1234,5,6,7,8,5678,9,10,11,12,9101112,123456789101112,13,14,15,16,13141516,17,18,19,20,17181920,21,22,23,24,21222324,131415161718192021222324,123456789101112131415161718192021222324".to_owned()));
}

#[test]
fn starts_with_attribute() {
    assert_eq!(find("[value^='1']"), (17, "1,1234,10,11,12,123456789101112,13,14,15,16,13141516,17,18,19,17181920,131415161718192021222324,123456789101112131415161718192021222324".to_owned()));
}

#[test]
fn ends_with_attribute() {
    assert_eq!(
        find("[value$='2']"),
        (5, "2,12,9101112,123456789101112,22".to_owned())
    );
}

#[test]
fn contains_attribute() {
    assert_eq!(find("[value*='2']"), (14, "2,1234,12,9101112,123456789101112,20,17181920,21,22,23,24,21222324,131415161718192021222324,123456789101112131415161718192021222324".to_owned()));
}

#[test]
fn greater_than_or_equal_attribute() {
    assert_eq!(
        find("[endIndex>=35]"),
        (
            6,
            "22,23,24,21222324,131415161718192021222324,123456789101112131415161718192021222324"
                .to_owned()
        )
    );
}

#[test]
fn less_than_or_equal_attribute() {
    assert_eq!(find("[endIndex<=5]"), (6, "1,2,3,4,1234,5".to_owned()));
}

#[test]
fn greater_than_attribute() {
    assert_eq!(
        find("[endIndex>35]"),
        (
            5,
            "23,24,21222324,131415161718192021222324,123456789101112131415161718192021222324"
                .to_owned()
        )
    );
}

#[test]
fn less_than_attribute() {
    assert_eq!(find("[endIndex<5]"), (5, "1,2,3,4,1234".to_owned()));
}

#[test]
fn or_selector() {
    assert_eq!(find("[value='5'], [value='6']"), (2, "5,6".to_owned()));
}

#[test]
fn nested_or_selector() {
    assert_eq!(
        find("parent > [value='5'], [value='6']"),
        (2, "5,6".to_owned())
    );
}

// ─── Beyond v12's tests: each checked against v12 on the same tree ──────

/// The same selector's `filter`, `not` and `parents` over every parent and
/// child (in `find_all` order): the values each returns, and how many `not`
/// keeps.
fn narrow(selector: &str) -> (String, usize, String) {
    let mut tree = EditableTree::new();
    let root = family_tree(&mut tree);
    let nodes = tree.find_all(root, |t, n| matches!(t.name(n), "child" | "parent"));
    let selector = Selector::new(selector).unwrap();
    (
        joined(&tree, &selector.filter(&tree, &nodes)),
        selector.not(&tree, &nodes).len(),
        joined(&tree, &selector.parents(&tree, &nodes)),
    )
}

fn error(selector: &str) -> String {
    Selector::new(selector).unwrap_err().to_string()
}

#[test]
fn an_invalid_selector_is_an_error_value_with_v12s_message() {
    assert_eq!(
        error("a >"),
        "[Invalid Selector] Error at line 1, column 3. Hint: *,[,*,["
    );
    assert_eq!(
        error(""),
        "[Invalid Selector] Error at line 1, column 1. Hint: *, [, *, ["
    );
    assert_eq!(
        error("child["),
        "[Invalid Selector] Error at line 1, column 6. Hint: "
    );
    assert_eq!(
        error("a,,b"),
        "[Invalid Selector] Error at line 1, column 3. Hint: *,["
    );
    assert_eq!(
        error("[ value = '4' ]"),
        "[Invalid Selector] Error at line 1, column 14. Hint: ]"
    );
}

#[test]
fn an_or_selector_has_exactly_two_parts() {
    assert_eq!(
        error("child, parent, grand-parent"),
        "[Invalid Selector] Error at line 1, column 16. Hint: +,~,>"
    );
}

#[test]
fn a_number_never_equals_text_and_always_differs_from_it() {
    assert_eq!(find("[endIndex='5']"), (0, String::new()));
    assert_eq!(find("[endIndex!='5']").0, 33);
}

#[test]
fn a_number_compares_with_text_converted_as_js_converts_it() {
    assert_eq!(
        find("[endIndex<'1e1']"),
        (11, "1,2,3,4,1234,5,6,7,8,5678,9".to_owned())
    );
    assert_eq!(find("[endIndex<' 7 ']"), (7, "1,2,3,4,1234,5,6".to_owned()));
    // "" is 0; "abc" is NaN, which nothing is less than.
    assert_eq!(find("[endIndex<'']").0, 0);
    assert_eq!(find("[endIndex<abc]").0, 0);
    assert_eq!(find("[endIndex>-1]").0, 33);
    assert_eq!(find("[lastIndex<=3]"), (5, "1,2,3,4,1234".to_owned()));
    // A number's text is searched as JS prints it.
    assert_eq!(
        find("[endIndex^='1']"),
        (8, "1,10,11,12,9101112,123456789101112,13,14".to_owned())
    );
}

#[test]
fn text_compares_with_text_character_by_character() {
    // "9" > "5" and "10" < "5": strings, so not by number.
    assert_eq!(find("[value>'5']"), (6, "6,7,8,5678,9,9101112".to_owned()));
    // An unquoted number stays text when the property is text.
    assert_eq!(find("[value>=5]").0, 7);
    assert_eq!(find("[name<'d']").0, 24);
}

#[test]
fn booleans_nodes_and_lists_read_as_js_reads_them() {
    // `true === "true"` is false, but `String(true)` starts with "t".
    assert_eq!(find("[isLeaf='true']").0, 0);
    assert_eq!(find("[isLeaf^='t']").0, 24);
    assert_eq!(find("[hasChildren>=1]").0, 9);
    // A parent reads as its value, but is never `===` text; the root has none.
    assert_eq!(find("[parent='1234']").0, 0);
    assert_eq!(find("[parent!='x']").0, 32);
    assert_eq!(find("[parent^='12']").0, 9);
    // Children read as their values joined with commas.
    assert_eq!(find("[children*='4,5']"), (1, "123456789101112".to_owned()));
}

#[test]
fn an_unknown_property_matches_nothing_even_with_not_equal() {
    assert_eq!(find("[bogus!='x']").0, 0);
    assert_eq!(find("[_value='x']").0, 0);
}

#[test]
fn chained_attributes_test_the_first_and_name_the_inner_selector() {
    // v12 takes the outer name-selector's name as "child[value^='1']", which
    // no node has, so chaining never matches.
    assert_eq!(find("child[value^='1'][value$='2']").0, 0);
}

#[test]
fn a_name_selector_inside_an_or_selector_narrows_before_it() {
    // v12 runs `child[value='4']` on its own first; `parent` then finds
    // nothing under child 4.
    assert_eq!(find("child[value='4'], parent"), (1, "4".to_owned()));
    // `*` does the same, and the or-selector then runs from every node.
    assert_eq!(find("*, child").0, 33);
}

#[test]
fn a_node_reached_twice_is_listed_twice() {
    let (count, values) = find("child ~ child");
    assert_eq!(count, 36);
    assert!(values.starts_with("2,3,4,3,4,4,6"));
    assert_eq!(
        find("parent ~ parent").1,
        "5678,9101112,9101112,17181920,21222324,21222324"
    );
}

#[test]
fn combinators_chain_left_to_right() {
    assert_eq!(
        find("grand-parent > parent child[value$='1']"),
        (3, "1,11,21".to_owned())
    );
    assert_eq!(find("parent > child + child").0, 18);
    assert_eq!(find("child[value='3'] ~ child"), (1, "4".to_owned()));
    assert_eq!(find("[value='4'] + *").0, 0);
    assert_eq!(find("* > * > *").0, 30);
    assert_eq!(find("* *").0, 119);
}

#[test]
fn filter_not_and_parents_run_from_the_root() {
    assert_eq!(
        narrow("[value>'5']"),
        (
            "6,7,8,5678,9,9101112".to_owned(),
            24,
            "5678,9101112".to_owned()
        )
    );
    // `root > *` reaches only grand-parents: none of the nodes, but their
    // nearest ancestors.
    assert_eq!(
        narrow("root > *"),
        (
            String::new(),
            30,
            "123456789101112,131415161718192021222324".to_owned()
        )
    );
    assert_eq!(
        narrow("parent ~ parent"),
        (
            "5678,9101112,9101112,17181920,21222324,21222324".to_owned(),
            26,
            "5678,9101112,17181920,21222324".to_owned()
        )
    );
}

#[test]
fn filter_not_and_parents_of_nothing_are_nothing() {
    let mut tree = EditableTree::new();
    family_tree(&mut tree);
    let selector = Selector::new("*").unwrap();
    let none: [NodeRef; 0] = [];
    assert!(selector.filter(&tree, &none).is_empty());
    assert!(selector.not(&tree, &none).is_empty());
    assert!(selector.parents(&tree, &none).is_empty());
}

#[test]
fn index_attributes_are_byte_offsets() {
    // DESIGN §21: the tree's offsets are bytes (§5), so on non-ASCII text
    // an index attribute names other numbers than v12's UTF-16 ones. "b"
    // after "é " starts at byte 3 and the space at 2; in v12 they start at
    // 2 and 1, and the text ends at 3.
    let mut pattern = common::seq(
        "s",
        vec![
            common::lit("e", "é"),
            common::lit("space", " "),
            common::lit("b", "b"),
        ],
    );
    let result = pattern.exec("é b");
    let tree = result.tree().unwrap().to_mut();
    let root = tree.root().unwrap();

    let at = |selector: &str| {
        let found = Selector::new(selector).unwrap().find(&tree, &[root]);
        joined(&tree, &found)
    };
    // v12: "b" alone, and nothing.
    assert_eq!(at("[startIndex>=2]"), " ,b");
    assert_eq!(at("[endIndex>3]"), "b,é b");
}
