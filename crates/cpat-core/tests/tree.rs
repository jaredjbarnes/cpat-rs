mod common;

use common::*;

fn names<'a>(nodes: impl IntoIterator<Item = cpat_core::Node<'a, 'a>>) -> Vec<&'static str> {
    nodes.into_iter().map(|n| n.name()).collect()
}

#[test]
fn values_are_slices_of_the_input() {
    let mut full_name = seq(
        "full-name",
        vec![
            lit("first", "John"),
            re("space", r"\s+"),
            lit("last", "Doe"),
        ],
    );
    let text = String::from("John   Doe");

    let result = full_name.exec(&text);
    let tree = result.tree().unwrap();
    let root = tree.root();

    assert_eq!(root.value(), "John   Doe");
    let space = root.children().nth(1).unwrap();
    assert_eq!(space.value(), "   ");
    assert!(
        std::ptr::eq(space.value().as_ptr(), text[4..].as_ptr()),
        "no copy: the value points into the input"
    );
}

#[test]
fn parents_and_children_link_up() {
    let mut pattern = seq(
        "outer",
        vec![seq("inner", vec![lit("a", "a")]), lit("b", "b")],
    );
    let result = pattern.exec("ab");
    let tree = result.tree().unwrap();

    let a = tree.root().find(|n| n.name() == "a").unwrap();
    assert_eq!(a.parent().unwrap().name(), "inner");
    assert_eq!(a.parent().unwrap().parent().unwrap().name(), "outer");
    assert!(tree.root().parent().is_none());
    assert_eq!(names(tree.root().children()), ["inner", "b"]);
}

#[test]
fn find_visits_children_before_parents() {
    let mut pattern = seq(
        "outer",
        vec![seq("inner", vec![lit("a", "a")]), lit("b", "b")],
    );
    let result = pattern.exec("ab");
    let root = result.tree().unwrap().root();

    // v12's `find` keeps testing the first match's ancestors, and the
    // outermost one that matches wins (`ast.find(() => true)` is `outer`).
    assert_eq!(root.find(|_| true).unwrap().name(), "outer");
    assert_eq!(root.find(|n| n.is_leaf()).unwrap().name(), "a");
    assert_eq!(names(root.find_all(|_| true)), ["a", "inner", "b", "outer"]);
    assert_eq!(names(root.find_all(|n| n.is_leaf())), ["a", "b"]);
    assert!(root.find(|n| n.name() == "missing").is_none());
}

#[test]
fn nodes_from_failed_alternatives_are_not_reachable() {
    // The first alternative builds an "a" node before failing on "b".
    let mut pattern = choice(
        "x",
        vec![
            seq("ab", vec![lit("a", "a"), lit("b", "b")]),
            seq("ac", vec![lit("a2", "a"), lit("c", "c")]),
        ],
    );
    let result = pattern.exec("ac");
    let root = result.tree().unwrap().root();

    assert_eq!(names(root.find_all(|_| true)), ["a2", "c", "ac"]);
}
