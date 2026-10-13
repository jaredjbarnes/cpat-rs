//! A port of clarity-pattern-parser v12's `src/query/query.test.ts`, every
//! test, then what v12 does that its tests don't pin (checked against v12).
//! v12's `$(root, selector)` is `Query::select(&tree, root, selector)` and
//! `$(root)` is `Query::of(root)`; a node's `toBe` identity is `NodeRef`
//! equality.
//!
//! - Construct → `construct`
//! - First → `first`
//! - Last → `last`
//! - Parent → `parent`
//! - Parents → `parents`
//! - Filter → `filter`
//! - Not → `not`
//! - First Before → `first_before`
//! - Last After → `last_after`
//! - Append → `append`
//! - Prepend → `prepend`
//! - ReplaceWith → `replace_with`
//! - Compact → `compact`
//! - Slice → `slice`
//! - End → `end`

mod common;

use common::{family_tree, joined};
use cpat_core::{EditableTree, NodeRef, Query};

fn family() -> (EditableTree<'static>, NodeRef) {
    let mut tree = EditableTree::new();
    let root = family_tree(&mut tree);
    (tree, root)
}

fn values(tree: &EditableTree<'_>, query: &Query) -> String {
    joined(tree, &query.to_array())
}

fn named(tree: &EditableTree<'_>, root: NodeRef, name: &str) -> Option<NodeRef> {
    tree.find(root, |t, n| t.name(n) == name)
}

fn valued(tree: &EditableTree<'_>, root: NodeRef, value: &str) -> Option<NodeRef> {
    tree.find(root, |t, n| t.value(n) == value)
}

#[test]
fn construct() {
    let (tree, root) = family();
    let children = Query::select(&tree, root, "child").unwrap();
    assert_eq!(
        values(&tree, &children),
        "1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20,21,22,23,24"
    );
}

#[test]
fn first() {
    let (tree, root) = family();
    let children = Query::select(&tree, root, "child").unwrap().first();
    assert_eq!(values(&tree, &children), "1");
}

#[test]
fn last() {
    let (tree, root) = family();
    let children = Query::select(&tree, root, "child").unwrap().last();
    assert_eq!(values(&tree, &children), "24");
}

#[test]
fn parent() {
    let (tree, root) = family();
    let parents = Query::select(&tree, root, "child").unwrap().parent(&tree);
    assert_eq!(parents.length(), 6);
    assert_eq!(
        values(&tree, &parents),
        "1234,5678,9101112,13141516,17181920,21222324"
    );
}

#[test]
fn parents() {
    let (tree, root) = family();
    let parents = Query::select(&tree, root, "child")
        .unwrap()
        .parents(&tree, "grand-parent parent[value='1234']")
        .unwrap();
    assert_eq!(parents.length(), 1);
    assert_eq!(values(&tree, &parents), "1234");
}

#[test]
fn filter() {
    let (tree, root) = family();
    let parents = Query::select(&tree, root, "parent")
        .unwrap()
        .filter(&tree, "grand-parent parent[value='1234']")
        .unwrap();
    assert_eq!(parents.length(), 1);
    assert_eq!(values(&tree, &parents), "1234");
}

#[test]
fn not() {
    let (tree, root) = family();
    let parents = Query::select(&tree, root, "parent")
        .unwrap()
        .not(&tree, "grand-parent parent[value='1234']")
        .unwrap();
    assert_eq!(parents.length(), 5);
    assert_eq!(
        values(&tree, &parents),
        "5678,9101112,13141516,17181920,21222324"
    );
}

#[test]
fn first_before() {
    let (mut tree, root) = family();
    Query::select(&tree, root, "child")
        .unwrap()
        .first()
        .before(&mut tree, |t, _| {
            t.create_value_node("node", "adopted-child", "0")
        });

    let adopted = named(&tree, root, "adopted-child").unwrap();
    let first_child = valued(&tree, root, "1");
    assert_eq!(tree.next_sibling(adopted), first_child);
}

#[test]
fn last_after() {
    let (mut tree, root) = family();
    Query::select(&tree, root, "child")
        .unwrap()
        .last()
        .after(&mut tree, |t, _| {
            t.create_value_node("node", "adopted-child", "25")
        });

    let adopted = named(&tree, root, "adopted-child").unwrap();
    let last_child = valued(&tree, root, "24");
    assert_eq!(tree.previous_sibling(adopted), last_child);
}

#[test]
fn append() {
    let (mut tree, root) = family();
    let parents = Query::select(&tree, root, "parent")
        .unwrap()
        .append(&mut tree, |t, n| {
            let last = *t.children(n).last().unwrap();
            let value = t.to_string(last).parse::<i32>().unwrap() + 1;
            t.create_value_node("node", "adopted-child", value.to_string())
        })
        .to_array();

    assert!(parents.iter().all(|&n| tree.children(n).len() == 5));
    assert_eq!(
        joined(&tree, &parents),
        "12345,56789,910111213,1314151617,1718192021,2122232425"
    );
}

#[test]
fn prepend() {
    let (mut tree, root) = family();
    let parents = Query::select(&tree, root, "parent")
        .unwrap()
        .prepend(&mut tree, |t, n| {
            let first = t.children(n)[0];
            let value = t.to_string(first).parse::<i32>().unwrap() - 1;
            t.create_value_node("node", "adopted-child", value.to_string())
        })
        .to_array();

    assert!(parents.iter().all(|&n| tree.children(n).len() == 5));
    assert_eq!(
        joined(&tree, &parents),
        "01234,45678,89101112,1213141516,1617181920,2021222324"
    );
}

#[test]
fn replace_with() {
    let (mut tree, root) = family();
    Query::select(&tree, root, "child")
        .unwrap()
        .replace_with(&mut tree, |t, _| t.create_value_node("node", "child", "b"));

    assert_eq!(tree.to_string(root), "bbbbbbbbbbbbbbbbbbbbbbbb");
}

#[test]
fn compact() {
    let (mut tree, root) = family();
    let length = Query::select(&tree, root, "parent")
        .unwrap()
        .compact(&mut tree)
        .find(&tree, "child")
        .unwrap()
        .length();

    assert_eq!(length, 0);
}

#[test]
fn slice() {
    let (tree, root) = family();
    let array = Query::select(&tree, root, "parent")
        .unwrap()
        .slice(0, Some(1))
        .to_array();

    assert_eq!(array.len(), 1);
    assert_eq!(tree.to_string(array[0]), "1234");
}

#[test]
fn end() {
    let (tree, root) = family();
    let grand_parents = Query::select(&tree, root, "grand-parent").unwrap();
    let parents = grand_parents.find(&tree, "parent").unwrap();
    let children = parents.find(&tree, "child").unwrap();

    assert_eq!(children.end().to_array(), parents.to_array());
    assert_eq!(parents.end().to_array(), grand_parents.to_array());
    assert_eq!(grand_parents.end().to_array(), [root]);
    assert_eq!(grand_parents.end().end().to_array(), [root]);
}

// ─── Beyond v12's tests ─────────────────────────────────────────────────

#[test]
fn a_bad_selector_is_an_error_value() {
    let (tree, root) = family();
    let error = Query::select(&tree, root, "child >").unwrap_err();
    assert_eq!(
        error.message,
        "[Invalid Selector] Error at line 1, column 7. Hint: *,[,*,["
    );
    assert!(Query::of(root).filter(&tree, "[").is_err());
    assert!(Query::of(root).not(&tree, "").is_err());
    assert!(Query::of(root).parents(&tree, "a,,b").is_err());
}

#[test]
fn set_value_replaces_each_node_with_a_leaf_of_its_type_and_name() {
    let (mut tree, root) = family();
    Query::select(&tree, root, "parent")
        .unwrap()
        .first()
        .set_value(&mut tree, "x");

    let first = tree.children(tree.children(root)[0])[0];
    assert_eq!(tree.name(first), "parent");
    assert_eq!(tree.type_name(first), "node");
    assert!(tree.is_leaf(first));
    assert_eq!(tree.to_string(root), "x56789101112131415161718192021222324");
}

#[test]
fn remove_detaches_and_normalize_renumbers_the_whole_tree() {
    let (mut tree, root) = family();
    let children = Query::select(&tree, root, "parent child").unwrap();
    let first_three = children.slice(0, Some(3));
    first_three.remove(&mut tree);
    assert_eq!(tree.parent(first_three.to_array()[0]), None);
    assert_eq!(tree.to_string(root), "456789101112131415161718192021222324");

    // From a child, the root's whole tree is renumbered: child 4 now starts it.
    children.slice(3, Some(4)).normalize(&mut tree);
    let four = valued(&tree, root, "4").unwrap();
    assert_eq!((tree.start_index(four), tree.end_index(four)), (0, 1));
    assert_eq!(tree.end_index(root), 36);
}

#[test]
fn slice_takes_js_indices_and_forgets_where_it_came_from() {
    let (tree, root) = family();
    let parents = Query::select(&tree, root, "parent").unwrap();
    assert_eq!(values(&tree, &parents.slice(-2, None)), "17181920,21222324");
    assert_eq!(values(&tree, &parents.slice(1, Some(-4))), "5678");
    assert_eq!(parents.slice(4, Some(2)).length(), 0);
    assert_eq!(parents.slice(-100, Some(100)).length(), 6);

    let sliced = parents.slice(0, Some(1));
    assert_eq!(sliced.end().to_array(), sliced.to_array());
    // `first` and `last` do remember.
    assert_eq!(parents.first().end().to_array(), parents.to_array());
}

#[test]
fn edits_that_need_a_parent_skip_the_root() {
    let (mut tree, root) = family();
    let mut visited = Vec::new();
    let query = Query::of(root);
    let mut visitor = |t: &mut EditableTree<'static>, n| {
        visited.push(n);
        t.create_value_node("node", "x", "x")
    };
    query.append(&mut tree, &mut visitor);
    query.prepend(&mut tree, &mut visitor);
    query.after(&mut tree, &mut visitor);
    query.before(&mut tree, &mut visitor);
    assert!(visited.is_empty());
    assert_eq!(tree.children(root).len(), 2);

    // `replace_with` calls its visitor for the root, and the root stays.
    query.replace_with(&mut tree, |t, n| {
        visited.push(n);
        t.create_value_node("node", "x", "x")
    });
    assert_eq!(visited, [root]);
    assert_eq!(tree.children(root).len(), 2);
}

#[test]
fn prepend_on_a_leaf_appends() {
    let (mut tree, root) = family();
    let one = valued(&tree, root, "1").unwrap();
    Query::new(vec![one]).prepend(&mut tree, |t, _| t.create_value_node("node", "x", "x"));
    assert_eq!(tree.to_string(one), "x");
    assert_eq!(tree.children(one).len(), 1);
}

#[test]
fn parent_lists_each_parent_once_and_skips_the_root() {
    let (tree, root) = family();
    let parents = Query::select(&tree, root, "*").unwrap().parent(&tree);
    // Children, parents and grand-parents give 6 + 2 + 1 parents.
    assert_eq!(parents.length(), 9);
    assert_eq!(parents.end().length(), 33);
}

/// A parent `p` over leaves a, b, c, and a node whose parent is `p` though
/// `p` doesn't list it: v12's `insertBefore` adopts even when the reference
/// node isn't a child.
fn with_stray() -> (EditableTree<'static>, NodeRef, NodeRef) {
    let mut tree = EditableTree::new();
    let leaves: Vec<_> = ["a", "b", "c"]
        .map(|v| tree.create_value_node("node", "k", v))
        .to_vec();
    let p = tree.create_node("node", "p", &leaves);
    let stray = tree.create_value_node("node", "stray", "s");
    let elsewhere = tree.create_value_node("node", "z", "z");
    tree.insert_before(p, stray, Some(elsewhere));
    (tree, p, stray)
}

#[test]
fn after_and_before_a_node_its_parent_does_not_list_use_js_index_minus_one() {
    // `indexOf` is -1: `after` splices at 0, `before` at -1 (before the last).
    let (mut tree, p, stray) = with_stray();
    Query::new(vec![stray]).after(&mut tree, |t, _| t.create_value_node("node", "n", "A"));
    assert_eq!(tree.to_string(p), "Aabc");

    let (mut tree, p, stray) = with_stray();
    Query::new(vec![stray]).before(&mut tree, |t, _| t.create_value_node("node", "n", "B"));
    assert_eq!(tree.to_string(p), "abBc");
}

#[test]
fn siblings_of_a_node_its_parent_does_not_list() {
    let (tree, _, stray) = with_stray();
    let after = cpat_core::Selector::new("stray ~ *")
        .unwrap()
        .find(&tree, &[stray]);
    assert_eq!(joined(&tree, &after), "a,b,c");
    let adjacent = cpat_core::Selector::new("stray + *")
        .unwrap()
        .find(&tree, &[stray]);
    assert!(adjacent.is_empty());
}
