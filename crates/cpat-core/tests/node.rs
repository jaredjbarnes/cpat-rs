//! A port of clarity-pattern-parser v12's `src/ast/Node.test.ts`, every test,
//! against the editable tree. v12's `new Node(type, name, first, last,
//! children, value)` is `new_node(type, name, first..last + 1, children,
//! value)`; its `toBe` identity is `NodeRef` equality.
//!
//! - Create → `create`
//! - Properties → `properties`
//! - Create Tree → `create_tree`
//! - Remove Child → `remove_child`
//! - Remove all Child → `remove_all_child`
//! - Replace Child → `replace_child`
//! - Insert Child → `insert_child`
//! - Insert Child With Null Reference → `insert_child_with_null_reference`
//! - Append Child → `append_child`
//! - Splice Children → `splice_children`
//! - Splice Children (Replace) → `splice_children_replace`
//! - Remove → `remove`
//! - Next Sibling → `next_sibling`
//! - Next Sibling (No Parent) → `next_sibling_no_parent`
//! - Next Sibling (Last Child) → `next_sibling_last_child`
//! - Previous Sibling → `previous_sibling`
//! - Previous Sibling (No Parent) → `previous_sibling_no_parent`
//! - Previous Sibling (First Child) → `previous_sibling_first_child`
//! - Find → `find`
//! - Walk Down → `walk_down`
//! - Walk Up → `walk_up`
//! - Clone → `clone`
//! - Turn Into JSON → `turn_into_json`
//! - Compact → `compact`
//! - Flatten → `flatten`
//! - Find Ancester → `find_ancestor`
//! - Find Ancester Without Parent → `find_ancestor_without_parent`
//! - Normalize values and index → `normalize_values_and_index`
//! - Normalize values and index deep → `normalize_values_and_index_deep`
//! - Normalize values with no values → `normalize_values_with_no_values`
//! - ReplaceWith → `replace_with`
//! - Transform → `transform`
//! - Find All → `find_all`
//! - Breadth First Early Exit → `breadth_first_early_exit`
//! - Find All (Breadth First) → `find_all_breadth_first`
//! - isEqual matches a composite node whose children were appended after
//!   construction → `is_equal_matches_a_composite_node_whose_children_were_appended_after_construction`
//! - isEqual is false when one node has extra trailing children →
//!   `is_equal_is_false_when_one_node_has_extra_trailing_children`

use cpat_core::{EditableTree, NodeRef, Visitors};

/// v12's `new Node(name, name, 0, 0, [], value)`, the leaf most tests make.
fn leaf(tree: &mut EditableTree<'static>, name: &'static str, value: &'static str) -> NodeRef {
    tree.create_value_node(name, name, value)
}

/// v12's `new Node(name, name, 0, 0, children)`: own value "".
fn parent_of(
    tree: &mut EditableTree<'static>,
    name: &'static str,
    children: &[NodeRef],
) -> NodeRef {
    tree.new_node(name, name, 0..1, children, "")
}

/// Two leaves `a` = "A" and `b` = "B" under `parent`.
fn a_b_parent(tree: &mut EditableTree<'static>) -> (NodeRef, NodeRef, NodeRef) {
    let a = leaf(tree, "a", "A");
    let b = leaf(tree, "b", "B");
    let parent = parent_of(tree, "parent", &[a, b]);
    (a, b, parent)
}

#[test]
fn create() {
    let mut tree = EditableTree::new();
    let node = tree.new_node("type", "name", 0..1, &[], "value");

    assert_eq!(tree.type_name(node), "type");
    assert_eq!(tree.name(node), "name");
    assert_eq!(tree.value(node), "value");
    assert_eq!(tree.first_index(node), 0);
    assert_eq!(tree.last_index(node), 0);
    assert_eq!(tree.start_index(node), 0);
    assert_eq!(tree.end_index(node), 1);
    assert_eq!(tree.parent(node), None);
    assert_eq!(tree.children(node).len(), 0);
}

#[test]
fn properties() {
    let mut tree = EditableTree::new();
    let child = leaf(&mut tree, "child", "Content");
    let parent = parent_of(&mut tree, "parent", &[child]);

    assert!(tree.has_children(parent));
    assert_eq!(tree.parent(child), Some(parent));
    assert_eq!(tree.children(parent)[0], child);
    assert_eq!(tree.type_name(child), "child");
    assert_eq!(tree.value(parent), "Content");
    assert_eq!(tree.value(child), "Content");
    assert_eq!(tree.name(child), "child");
    assert_eq!(tree.first_index(child), 0);
    assert_eq!(tree.last_index(child), 0);
    assert_eq!(tree.start_index(child), 0);
    assert_eq!(tree.end_index(child), 1);
}

#[test]
fn create_tree() {
    let mut tree = EditableTree::new();
    let child = leaf(&mut tree, "child", "Child");
    let parent = parent_of(&mut tree, "parent", &[child]);

    assert_eq!(tree.value(parent), tree.value(child));
}

#[test]
fn remove_child() {
    let mut tree = EditableTree::new();
    let child = leaf(&mut tree, "child", "Child");
    let parent = parent_of(&mut tree, "parent", &[child]);

    tree.remove_child(parent, child);

    assert_eq!(tree.children(parent).len(), 0);
    assert_eq!(tree.parent(child), None);
    assert_eq!(tree.value(parent), "");
    assert_eq!(tree.value(child), "Child");
}

#[test]
fn remove_all_child() {
    let mut tree = EditableTree::new();
    let a = leaf(&mut tree, "a", "A");
    let b = leaf(&mut tree, "b", "B");
    let parent = tree.new_node("parent", "parent", 0..1, &[a, b], "Fallback Value");

    assert_eq!(tree.children(parent).len(), 2);
    assert_eq!(tree.parent(a), Some(parent));
    assert_eq!(tree.parent(b), Some(parent));
    assert_eq!(tree.value(parent), "AB");
    assert_eq!(tree.value(a), "A");
    assert_eq!(tree.value(b), "B");

    tree.remove_all_children(parent);

    assert_eq!(tree.children(parent).len(), 0);
    assert_eq!(tree.parent(a), None);
    assert_eq!(tree.parent(b), None);
    assert_eq!(tree.value(parent), "Fallback Value");
    assert_eq!(tree.value(a), "A");
    assert_eq!(tree.value(b), "B");
}

#[test]
fn replace_child() {
    let mut tree = EditableTree::new();
    let a = leaf(&mut tree, "a", "A");
    let b = leaf(&mut tree, "b", "B");
    let parent = parent_of(&mut tree, "parent", &[a]);

    tree.replace_child(parent, b, a);

    assert_eq!(tree.children(parent).len(), 1);
    assert_eq!(tree.parent(a), None);
    assert_eq!(tree.parent(b), Some(parent));
    assert_eq!(tree.value(a), "A");
    assert_eq!(tree.value(b), "B");
}

#[test]
fn insert_child() {
    let mut tree = EditableTree::new();
    let a = leaf(&mut tree, "a", "A");
    let b = leaf(&mut tree, "b", "B");
    let parent = parent_of(&mut tree, "parent", &[a]);

    assert_eq!(tree.value(parent), "A");

    tree.insert_before(parent, b, Some(a));

    assert_eq!(tree.children(parent).len(), 2);
    assert_eq!(tree.value(parent), "BA");
    assert_eq!(tree.children(parent), &[b, a]);
    assert_eq!(tree.parent(a), Some(parent));
    assert_eq!(tree.parent(b), Some(parent));
    assert_eq!(tree.value(a), "A");
    assert_eq!(tree.value(b), "B");
}

#[test]
fn insert_child_with_null_reference() {
    let mut tree = EditableTree::new();
    let a = leaf(&mut tree, "a", "A");
    let b = leaf(&mut tree, "b", "B");
    let parent = parent_of(&mut tree, "parent", &[a]);

    assert_eq!(tree.value(parent), "A");

    tree.insert_before(parent, b, None);

    assert_eq!(tree.children(parent).len(), 2);
    assert_eq!(tree.value(parent), "AB");
    assert_eq!(tree.children(parent), &[a, b]);
    assert_eq!(tree.parent(a), Some(parent));
    assert_eq!(tree.parent(b), Some(parent));
    assert_eq!(tree.value(a), "A");
    assert_eq!(tree.value(b), "B");
}

#[test]
fn append_child() {
    let mut tree = EditableTree::new();
    let a = leaf(&mut tree, "a", "A");
    let b = leaf(&mut tree, "b", "B");
    let parent = parent_of(&mut tree, "parent", &[a]);

    assert_eq!(tree.value(parent), "A");

    tree.append_child(parent, b);

    assert_eq!(tree.children(parent).len(), 2);
    assert_eq!(tree.value(parent), "AB");
    assert_eq!(tree.children(parent), &[a, b]);
    assert_eq!(tree.parent(a), Some(parent));
    assert_eq!(tree.parent(b), Some(parent));
    assert_eq!(tree.value(a), "A");
    assert_eq!(tree.value(b), "B");
}

#[test]
fn splice_children() {
    let mut tree = EditableTree::new();
    let a = leaf(&mut tree, "a", "A");
    let b = leaf(&mut tree, "b", "B");
    let parent = parent_of(&mut tree, "parent", &[a]);

    assert_eq!(tree.value(parent), "A");

    tree.splice_children(parent, 0, 0, &[b]);

    assert_eq!(tree.children(parent).len(), 2);
    assert_eq!(tree.value(parent), "BA");
    assert_eq!(tree.children(parent), &[b, a]);
    assert_eq!(tree.parent(a), Some(parent));
    assert_eq!(tree.parent(b), Some(parent));
    assert_eq!(tree.value(a), "A");
    assert_eq!(tree.value(b), "B");
}

#[test]
fn splice_children_replace() {
    let mut tree = EditableTree::new();
    let a = leaf(&mut tree, "a", "A");
    let b = leaf(&mut tree, "b", "B");
    let parent = parent_of(&mut tree, "parent", &[a]);

    assert_eq!(tree.value(parent), "A");

    let removed = tree.splice_children(parent, 0, 1, &[b]);

    assert_eq!(removed, vec![a]);
    assert_eq!(tree.children(parent).len(), 1);
    assert_eq!(tree.value(parent), "B");
    assert_eq!(tree.children(parent)[0], b);
    assert_eq!(tree.parent(a), None);
    assert_eq!(tree.parent(b), Some(parent));
    assert_eq!(tree.value(a), "A");
    assert_eq!(tree.value(b), "B");
}

#[test]
fn remove() {
    let mut tree = EditableTree::new();
    let (a, b, parent) = a_b_parent(&mut tree);

    let found = tree.find(parent, |t, n| t.name(n) == "b").unwrap();
    tree.remove(found);

    assert_eq!(tree.children(parent).len(), 1);
    assert_eq!(tree.value(parent), "A");
    assert_eq!(tree.children(parent)[0], a);
    assert_eq!(tree.parent(a), Some(parent));
    assert_eq!(tree.parent(b), None);
    assert_eq!(tree.value(a), "A");
}

#[test]
fn next_sibling() {
    let mut tree = EditableTree::new();
    let (a, b, _) = a_b_parent(&mut tree);

    assert_eq!(tree.next_sibling(a), Some(b));
}

#[test]
fn next_sibling_no_parent() {
    let mut tree = EditableTree::new();
    let a = leaf(&mut tree, "a", "A");

    assert_eq!(tree.next_sibling(a), None);
}

#[test]
fn next_sibling_last_child() {
    let mut tree = EditableTree::new();
    let (_, b, _) = a_b_parent(&mut tree);

    assert_eq!(tree.next_sibling(b), None);
}

#[test]
fn previous_sibling() {
    let mut tree = EditableTree::new();
    let (a, b, _) = a_b_parent(&mut tree);

    assert_eq!(tree.previous_sibling(b), Some(a));
}

#[test]
fn previous_sibling_no_parent() {
    let mut tree = EditableTree::new();
    let a = leaf(&mut tree, "a", "A");

    assert_eq!(tree.previous_sibling(a), None);
}

#[test]
fn previous_sibling_first_child() {
    let mut tree = EditableTree::new();
    let (a, _, _) = a_b_parent(&mut tree);

    assert_eq!(tree.previous_sibling(a), None);
}

#[test]
fn find() {
    let mut tree = EditableTree::new();
    let (a, _, parent) = a_b_parent(&mut tree);

    assert_eq!(tree.find(parent, |t, n| t.name(n) == "a"), Some(a));
}

#[test]
fn walk_down() {
    let mut tree = EditableTree::new();
    let (a, b, parent) = a_b_parent(&mut tree);

    let mut result = Vec::new();
    tree.walk_down(parent, |_, n| {
        result.push(n);
        true
    });

    assert_eq!(result, vec![parent, a, b]);
}

#[test]
fn walk_up() {
    let mut tree = EditableTree::new();
    let (a, b, parent) = a_b_parent(&mut tree);

    let mut result = Vec::new();
    tree.walk_up(parent, |_, n| {
        result.push(n);
        true
    });

    assert_eq!(result, vec![a, b, parent]);
}

#[test]
fn clone() {
    let mut tree = EditableTree::new();
    let (_, _, parent) = a_b_parent(&mut tree);

    let clone = tree.clone_node(parent);

    assert_ne!(clone, parent);
    assert!(tree.is_equal(clone, &tree, parent));
}

#[test]
fn turn_into_json() {
    let mut tree = EditableTree::new();
    let (_, _, parent) = a_b_parent(&mut tree);

    let result = tree.to_json(parent, 0);

    let id = |n: NodeRef| tree.id(n);
    let [a, b] = [tree.children(parent)[0], tree.children(parent)[1]];
    let expected = format!(
        concat!(
            r#"{{"id":"{}","type":"parent","name":"parent","value":"AB","startIndex":0,"endIndex":1,"children":["#,
            r#"{{"id":"{}","type":"a","name":"a","value":"A","startIndex":0,"endIndex":1,"children":[]}},"#,
            r#"{{"id":"{}","type":"b","name":"b","value":"B","startIndex":0,"endIndex":1,"children":[]}}]}}"#,
        ),
        id(parent),
        id(a),
        id(b),
    );
    assert_eq!(result, expected);
}

#[test]
fn compact() {
    let mut tree = EditableTree::new();
    let child = tree.new_node("child", "child", 0..7, &[], "Content");
    let parent = tree.new_node("parent", "parent", 0..7, &[child], "");

    tree.compact(parent);

    assert!(!tree.has_children(parent));
    assert_eq!(tree.value(parent), "Content");
}

#[test]
fn flatten() {
    let mut tree = EditableTree::new();
    let a = tree.new_node("a", "a", 0..1, &[], "A");
    let b = tree.new_node("b", "b", 1..2, &[], "B");
    let c = tree.new_node("c", "c", 2..3, &[], "C");
    let parent = tree.new_node("parent", "parent", 0..2, &[a, b], "");
    let grand_parent = tree.new_node("grand-parent", "grand-parent", 0..3, &[parent, c], "");

    assert_eq!(tree.flatten(grand_parent), vec![a, b, c]);
}

#[test]
fn find_ancestor() {
    let mut tree = EditableTree::new();
    let child = parent_of(&mut tree, "child", &[]);
    let parent = parent_of(&mut tree, "parent", &[child]);
    let grand_parent = parent_of(&mut tree, "grand-parent", &[parent]);

    let result = tree.find_ancestor(child, |t, p| t.name(p) == "grand-parent");

    assert_eq!(result, Some(grand_parent));
}

#[test]
fn find_ancestor_without_parent() {
    let mut tree = EditableTree::new();
    let child = parent_of(&mut tree, "child", &[]);

    assert_eq!(
        tree.find_ancestor(child, |t, p| t.name(p) == "parent"),
        None
    );
}

#[test]
fn normalize_values_and_index() {
    let mut tree = EditableTree::new();
    let first = tree.create_value_node("literal", "first", "first");
    let second = tree.create_value_node("literal", "second", "second");
    let parent = tree.new_node("literal", "parent", 0..1, &[], "");

    tree.append_child(parent, first);
    tree.append_child(parent, second);

    tree.normalize(parent);

    assert_eq!(tree.start_index(parent), 0);
    assert_eq!(tree.first_index(parent), 0);
    assert_eq!(tree.last_index(parent), 10);
    assert_eq!(tree.end_index(parent), 11);
}

#[test]
fn normalize_values_and_index_deep() {
    let mut tree = EditableTree::new();
    let first = tree.create_value_node("literal", "first", "first");
    let second = tree.create_value_node("literal", "second", "second");
    let grand_child = tree.create_value_node("literal", "three", "three");
    let parent = tree.new_node("literal", "parent", 0..1, &[], "");

    tree.append_child(parent, first);
    tree.append_child(parent, second);

    tree.normalize(parent);

    tree.append_child(second, grand_child);
    tree.normalize(parent);

    assert_eq!(tree.start_index(parent), 0);
    assert_eq!(tree.first_index(parent), 0);
    assert_eq!(tree.last_index(parent), 9);
    assert_eq!(tree.end_index(parent), 10);
}

#[test]
fn normalize_values_with_no_values() {
    let mut tree = EditableTree::new();
    let node = tree.create_value_node("literal", "node", "");

    tree.normalize(node);

    assert_eq!(tree.start_index(node), 0);
    assert_eq!(tree.first_index(node), 0);
    assert_eq!(tree.last_index(node), 0);
    assert_eq!(tree.end_index(node), 1);
}

#[test]
fn replace_with() {
    let mut tree = EditableTree::new();
    let first_leaf = tree.create_value_node("node", "first", "first");
    let second_leaf = tree.create_value_node("node", "second", "second");
    let parent = tree.new_node("node", "parent", 0..1, &[first_leaf, second_leaf], "");

    let first = tree.find(parent, |t, n| t.name(n) == "first").unwrap();
    let new_node = tree.create_value_node("node", "new", "new");
    tree.replace_with(first, new_node);

    let result = tree.find(parent, |t, n| t.name(n) == "new").unwrap();

    assert_eq!(result, new_node);
    assert_eq!(tree.parent(first), None);
    assert_eq!(tree.parent(result), Some(parent));
}

#[test]
fn transform() {
    let mut tree = EditableTree::new();
    let john = tree.create_value_node("family", "child", "John");
    let jane = tree.create_value_node("family", "child", "Jane");
    let jack = tree.create_value_node("family", "child", "Jack");
    let parent = tree.create_node("family", "parent", &[john, jane, jack]);
    let aunt = tree.create_value_node("family", "aunt", "aunt");
    let node = tree.create_node("family", "grandparent", &[parent, aunt]);

    let mut visitors = Visitors::new()
        .on("child", |t: &mut EditableTree<'static>, n| {
            let value = t.to_string(n);
            t.create_value_node("family", "adopted-child", value)
        })
        .on("parent", |t: &mut EditableTree<'static>, n| {
            let children = t.children(n).to_vec();
            t.create_node("family", "adopted-parent", &children)
        })
        .on("grandparent", |t: &mut EditableTree<'static>, n| {
            let children = t.children(n).to_vec();
            t.create_node("family", "adopted-grandparent", &children)
        });
    let result = tree.transform(node, &mut visitors);

    let john = tree.create_value_node("family", "adopted-child", "John");
    let jane = tree.create_value_node("family", "adopted-child", "Jane");
    let jack = tree.create_value_node("family", "adopted-child", "Jack");
    let parent = tree.create_node("family", "adopted-parent", &[john, jane, jack]);
    let aunt = tree.create_value_node("family", "aunt", "aunt");
    let expected = tree.create_node("family", "adopted-grandparent", &[parent, aunt]);

    assert!(tree.is_equal(result, &tree, expected));
}

#[test]
fn find_all() {
    let mut tree = EditableTree::new();
    let a = leaf(&mut tree, "a", "A");
    let b = leaf(&mut tree, "b", "B");
    let a2 = tree.clone_node(a);
    let parent = parent_of(&mut tree, "parent", &[a, b, a2]);

    assert_eq!(tree.find_all(parent, |t, n| t.name(n) == "a"), vec![a, a2]);
}

#[test]
fn breadth_first_early_exit() {
    let mut tree = EditableTree::new();
    let a = leaf(&mut tree, "a", "A");
    let b = leaf(&mut tree, "b", "B");
    let a2 = tree.clone_node(a);
    let parent = parent_of(&mut tree, "parent", &[a, b, a2]);

    let mut count = 0;
    tree.walk_breadth_first(parent, |t, n| {
        count += 1;
        t.name(n) != "a"
    });

    assert_eq!(count, 2);
}

#[test]
fn find_all_breadth_first() {
    let mut tree = EditableTree::new();
    let a = leaf(&mut tree, "a", "A");
    let b = leaf(&mut tree, "b", "B");
    let a2 = tree.clone_node(a);
    let parent = parent_of(&mut tree, "parent", &[a, b, a2]);

    assert_eq!(
        tree.find_all_breadth_first(parent, |t, n| t.name(n) == "a"),
        vec![a, a2]
    );
}

#[test]
fn is_equal_matches_a_composite_node_whose_children_were_appended_after_construction() {
    let mut tree = EditableTree::new();
    let built = tree.create_node("expression", "sum", &[]);
    let a = tree.create_value_node("literal", "a", "a");
    let plus = tree.create_value_node("literal", "+", "+");
    let b = tree.create_value_node("literal", "b", "b");
    tree.append(built, &[a, plus, b]);

    let a = tree.create_value_node("literal", "a", "a");
    let plus = tree.create_value_node("literal", "+", "+");
    let b = tree.create_value_node("literal", "b", "b");
    let expected = tree.create_node("expression", "sum", &[a, plus, b]);

    assert!(tree.is_equal(built, &tree, expected));
    assert!(tree.is_equal(expected, &tree, built));
}

#[test]
fn is_equal_is_false_when_one_node_has_extra_trailing_children() {
    let mut tree = EditableTree::new();
    let a = tree.create_value_node("literal", "a", "a");
    let shorter = tree.create_node("expression", "list", &[a]);
    let a = tree.create_value_node("literal", "a", "a");
    let b = tree.create_value_node("literal", "b", "b");
    let longer = tree.create_node("expression", "list", &[a, b]);

    assert!(!tree.is_equal(shorter, &tree, longer));
    assert!(!tree.is_equal(longer, &tree, shorter));
}
