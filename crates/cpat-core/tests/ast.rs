//! The editable tree on real parse trees, compared with clarity-pattern-parser
//! v12, and the read-only tree's share of the `Node` API.
//!
//! The JSON under `fixtures/ast/` was generated once with v12 (branch
//! `release/v12`) by a tsx script that runs `GRAMMAR` on `TEXT` with v12's
//! `Grammar.parseString`, then `ast.toJson(2)` (`pratt_parsed.json`), and on
//! a second parse `compact(ast, { group: true })`, `remove(ast, { spaces:
//! true })`, `ast.transform({ number: n => Node.createValueNode(
//! "number-literal", "num", n.value), add: n => Node.createNode("binary",
//! "sum", n.children.slice()) })`, `normalize()` and `toJson(2)`
//! (`pratt_edited.json`). The `escapes_*.json` files are `toJson(0 | 4 |
//! 11)` of the nodes `json_escapes_and_indents_as_json_stringify` builds,
//! and `pratt_heavy.json` is `toJson(2)` of the longer text that the
//! read-only/editable comparison parses. v12 numbers nodes from one counter
//! for the whole process, so its ids for a parse depend on everything parsed
//! before; the parse comparisons mask ids, the hand-built ones don't need to.

use cpat_core::ast::{compact, remove};
use cpat_core::{EditableTree, Visitors, patterns};

const GRAMMAR: &str = r#"
spaces = /\s+/
number = /\d+/
variable = /[a-z]+/
group = "(" + spaces? + expr + spaces? + ")"
neg = "-" + expr
fact = expr + "!"
pow = expr + "^" + expr
mul = expr + spaces? + "*" + spaces? + expr
add = expr + spaces? + "+" + spaces? + expr
expr = group | neg | fact | pow right | mul | add | number | variable
"#;

const TEXT: &str = "-(1 + 2) * 3!^x^2 + y";

/// Replaces every `"id": "<n>"` with `"id": "#"`.
fn mask_ids(json: &str) -> String {
    let ids = regex::Regex::new(r#""id": ?"\d+""#).unwrap();
    ids.replace_all(json, r##""id": "#""##).into_owned()
}

fn pattern(name: &str) -> cpat_core::Pattern {
    patterns(GRAMMAR).unwrap()[name].clone()
}

fn fixture(name: &str) -> String {
    let path = format!("{}/tests/fixtures/ast/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

#[test]
fn a_parse_tree_prints_v12_json() {
    let result = pattern("expr").exec(TEXT);
    let tree = result.tree().unwrap();

    let expected = fixture("pratt_parsed.json");
    assert_eq!(
        mask_ids(&tree.root().to_json(2)),
        mask_ids(expected.trim_end())
    );
    assert_eq!(
        mask_ids(&tree.to_mut().to_json(tree.to_mut().root().unwrap(), 2)),
        mask_ids(expected.trim_end())
    );
}

#[test]
fn compact_remove_transform_and_normalize_give_v12_json() {
    let result = pattern("expr").exec(TEXT);
    let mut tree = result.tree().unwrap().to_mut();
    let root = tree.root().unwrap();

    compact(&mut tree, root, &["group"]);
    remove(&mut tree, root, &["spaces"]);
    let mut visitors = Visitors::new()
        .on("number", |t: &mut EditableTree<'_>, n| {
            let value = t.to_string(n);
            t.create_value_node("number-literal", "num", value)
        })
        .on("add", |t: &mut EditableTree<'_>, n| {
            let children = t.children(n).to_vec();
            t.create_node("binary", "sum", &children)
        });
    let out = tree.transform(root, &mut visitors);
    tree.normalize(out);

    assert_eq!(
        mask_ids(&tree.to_json(out, 2)),
        mask_ids(fixture("pratt_edited.json").trim_end())
    );
    // The new nodes counted on from the parse's ids, so none repeats one.
    let mut ids: Vec<u32> = tree
        .find_all(out, |_, _| true)
        .iter()
        .map(|&n| tree.id(n))
        .collect();
    let count = ids.len();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), count);
}

#[test]
fn the_read_only_json_is_the_editable_json_for_a_pratt_heavy_parse() {
    let text = "-(1 + -2!) * 3!^x^2^(y * -z) + 4 * 5 + --6!!^7";
    let result = pattern("expr").exec(text);
    let tree = result.tree().unwrap();
    assert_eq!(tree.root().value(), text, "the whole text parses");
    assert!(
        tree.root()
            .find_all(|n| n.kind() == cpat_core::NodeKind::Expression)
            .len()
            > 10,
        "many expression nodes"
    );

    let editable = tree.to_mut();
    let root = editable.root().unwrap();
    for space in [0, 2, 4] {
        assert_eq!(
            tree.root().to_json(space),
            editable.to_json(root, space),
            "space {space}"
        );
    }
    assert_eq!(
        mask_ids(&tree.root().to_json(2)),
        mask_ids(fixture("pratt_heavy.json").trim_end())
    );
}

#[test]
fn to_mut_keeps_ids_kinds_ranges_and_parents() {
    let result = pattern("expr").exec(TEXT);
    let tree = result.tree().unwrap();
    let editable = tree.to_mut();

    let read_only = tree.root().find_all(|_| true);
    let edit = editable.find_all(editable.root().unwrap(), |_, _| true);
    assert_eq!(read_only.len(), edit.len());
    for (node, &n) in read_only.iter().zip(&edit) {
        assert_eq!(node.id().index(), editable.id(n));
        assert_eq!(node.kind().as_str(), editable.type_name(n));
        assert_eq!(node.name(), editable.name(n));
        assert_eq!(node.value(), editable.value(n));
        assert_eq!(
            (node.start(), node.end()),
            (editable.start_index(n), editable.end_index(n))
        );
        assert_eq!(
            node.parent().map(|p| p.id().index()),
            editable.parent(n).map(|p| editable.id(p))
        );
    }
}

#[test]
fn editing_leaves_the_parse_tree_alone() {
    let result = pattern("expr").exec(TEXT);
    let tree = result.tree().unwrap();
    let before = tree.root().to_json(0);

    let mut editable = tree.to_mut();
    let root = editable.root().unwrap();
    remove(&mut editable, root, &["spaces"]);

    assert_eq!(editable.value(root), "-(1+2)*3!^x^2+y");
    assert_eq!(tree.root().to_json(0), before);
}

#[test]
fn a_parent_left_with_no_children_reads_as_its_own_empty_value() {
    // v12's parse nodes for sequences keep "" as their own value, so a
    // parse parent stripped of its children reads as "".
    let result = pattern("group").exec("(1)");
    let mut tree = result.tree().unwrap().to_mut();
    let root = tree.root().unwrap();

    tree.remove_all_children(root);

    assert_eq!(tree.value(root), "");
}

#[test]
fn json_escapes_and_indents_as_json_stringify() {
    let mut tree = EditableTree::new();
    let a = tree.create_value_node(
        "t\"q",
        "n\\m",
        "\u{1}\"\\/\u{2028}\n\t\u{7f}\u{1f}é\u{8}\u{c}\r",
    );
    let e = tree.create_value_node("e", "e", "");
    let p = tree.create_node("p", "p", &[a, e]);

    assert_eq!(tree.to_json(p, 0), fixture("escapes_space0.json"));
    assert_eq!(tree.to_json(p, 4), fixture("escapes_space4.json"));
    assert_eq!(
        tree.to_json(p, 11),
        fixture("escapes_space11.json"),
        "capped at 10"
    );
}

#[test]
fn the_cycle_free_object_carries_the_json_fields() {
    let mut tree = EditableTree::new();
    let a = tree.new_node("literal", "a", 3..4, &[], "A");
    let b = tree.create_value_node("literal", "b", "B");
    let p = tree.create_node("seq", "p", &[a, b]);

    let object = tree.to_cycle_free_object(p);

    assert_eq!(
        (
            object.id.as_str(),
            object.type_name.as_str(),
            object.name.as_str()
        ),
        ("2", "seq", "p")
    );
    assert_eq!(object.value, "AB");
    assert_eq!(object.children.len(), 2);
    assert_eq!(object.children[0].value, "A");
    assert_eq!(
        (object.children[0].start_index, object.children[0].end_index),
        (3, 4)
    );
    assert_eq!(object, tree.to_cycle_free_object(p));
    assert_ne!(object, tree.to_cycle_free_object(a));
}

// ─── v12's walk semantics ───────────────────────────────────────────

/// `xp(xq(xa), xb)`, all names starting with "x".
fn x_tree() -> (EditableTree<'static>, [cpat_core::NodeRef; 4]) {
    let mut tree = EditableTree::new();
    let xa = tree.create_value_node("x", "xa", "A");
    let xq = tree.create_node("x", "xq", &[xa]);
    let xb = tree.create_value_node("x", "xb", "B");
    let xp = tree.create_node("x", "xp", &[xq, xb]);
    (tree, [xa, xq, xb, xp])
}

#[test]
fn a_false_in_walk_up_skips_siblings_but_still_visits_ancestors() {
    // v12: `['xa', 'xq', 'xp']`: `xb` is skipped, the ancestors are not.
    let (mut tree, [xa, xq, _, xp]) = x_tree();
    let mut seen = Vec::new();

    let finished = tree.walk_up(xp, |_, n| {
        seen.push(n);
        n != xa
    });

    assert_eq!(seen, vec![xa, xq, xp]);
    assert!(!finished);
}

#[test]
fn find_lets_a_matching_ancestor_take_over() {
    // v12's `find(n => n.name.startsWith("x"))` gives `xp`, not `xa`.
    let (tree, [xa, _, _, xp]) = x_tree();

    assert_eq!(tree.find(xp, |t, n| t.name(n).starts_with('x')), Some(xp));
    assert_eq!(tree.find(xp, |t, n| t.name(n) == "xa"), Some(xa));
    assert_eq!(
        tree.find_breadth_first(xp, |t, n| t.name(n).starts_with('x')),
        Some(xp)
    );
}

#[test]
fn a_false_in_walk_down_ends_the_walk() {
    let (mut tree, [xa, xq, xb, xp]) = x_tree();
    let mut seen = Vec::new();

    let finished = tree.walk_down(xp, |_, n| {
        seen.push(n);
        n != xq
    });

    assert_eq!(seen, vec![xp, xq]);
    assert!(!finished);
    let mut all = Vec::new();
    assert!(tree.walk_down(xp, |_, n| {
        all.push(n);
        true
    }));
    assert_eq!(all, vec![xp, xq, xa, xb]);
}

#[test]
fn a_walk_may_edit_the_tree_it_walks() {
    // Children are read before a node is entered (`walk_up`, `walk_down`),
    // so removing a node mid-walk still visits the rest of the old list.
    let (mut tree, [xa, xq, xb, xp]) = x_tree();
    let mut seen = Vec::new();

    tree.walk_up(xp, |t, n| {
        seen.push(n);
        if n == xa {
            t.remove(xb);
        }
        true
    });

    assert_eq!(seen, vec![xa, xq, xb, xp]);
    assert_eq!(tree.children(xp), &[xq]);
}

#[test]
fn compact_by_name_skips_what_it_compacted_and_remove_spares_the_root() {
    let (mut tree, [xa, xq, xb, xp]) = x_tree();
    let mut visited = Vec::new();
    tree.walk_breadth_first(xp, |_, n| {
        visited.push(n);
        true
    });
    assert_eq!(visited, vec![xp, xq, xb, xa]);

    compact(&mut tree, xp, &["xq"]);
    assert!(tree.is_leaf(xq));
    assert_eq!(tree.value(xq), "A");
    assert_eq!(tree.parent(xa), None);

    remove(&mut tree, xp, &["xp", "xb"]);
    assert_eq!(tree.children(xp), &[xq]);
    assert_eq!(tree.value(xp), "A");
}

// ─── Child edits at their edges ─────────────────────────────────────

#[test]
fn splice_clamps_like_array_splice() {
    let (mut tree, [_, xq, xb, xp]) = x_tree();
    let extra = tree.create_value_node("x", "extra", "E");

    let removed = tree.splice_children(xp, 1, 99, &[extra]);
    assert_eq!(removed, vec![xb]);
    assert_eq!(tree.children(xp), &[xq, extra]);

    let removed = tree.splice_children(xp, 99, 1, &[xb]);
    assert!(removed.is_empty());
    assert_eq!(tree.children(xp), &[xq, extra, xb]);
    assert_eq!(tree.parent(xb), Some(xp));
}

#[test]
fn insert_before_a_stranger_adopts_without_inserting() {
    // v12 sets the parent first and inserts only if the reference is a child.
    let (mut tree, [xa, xq, xb, xp]) = x_tree();
    let new = tree.create_value_node("x", "new", "N");

    tree.insert_before(xp, new, Some(xa));

    assert_eq!(tree.children(xp), &[xq, xb]);
    assert_eq!(tree.parent(new), Some(xp));
}

#[test]
fn find_root_and_ancestors_follow_parents() {
    let (tree, [xa, xq, _, xp]) = x_tree();

    assert_eq!(tree.find_root(xa), xp);
    assert_eq!(tree.find_root(xp), xp);
    assert_eq!(tree.find_ancestor(xa, |_, _| true), Some(xq));
    assert_eq!(
        tree.find_ancestor(xa, |t, n| t.name(n) == "xa"),
        None,
        "not itself"
    );
}

#[test]
fn clone_copies_deeply_with_new_ids_children_first() {
    let (mut tree, [_, xq, xb, xp]) = x_tree();

    let copy = tree.clone_node(xp);

    assert_eq!(tree.parent(copy), None);
    assert!(tree.is_equal(copy, &tree, xp));
    let copied_xq = tree.children(copy)[0];
    assert_ne!(copied_xq, xq);
    assert_eq!(tree.parent(copied_xq), Some(copy));
    // Children are made before their parent, as v12's `clone` does.
    assert!(tree.id(copied_xq) < tree.id(copy));
    assert!(tree.id(xb) < tree.id(copied_xq));
}

#[test]
fn normalize_from_a_start_lays_leaves_end_to_end() {
    let (mut tree, [xa, xq, xb, xp]) = x_tree();
    let empty = tree.create_value_node("x", "empty", "");
    tree.append_child(xq, empty);

    assert_eq!(tree.normalize_from(xp, 5), 2);

    let ranges: Vec<_> = [xp, xq, xa, empty, xb]
        .iter()
        .map(|&n| (tree.start_index(n), tree.end_index(n)))
        .collect();
    // The empty leaf starts and ends where `xb` starts: v12's `lastIndex`
    // of `start - 1`, made half-open.
    assert_eq!(ranges, vec![(5, 7), (5, 6), (5, 6), (6, 6), (6, 7)]);
    assert_eq!(tree.last_index(empty), 5);
}

#[test]
fn is_equal_sees_names_indices_and_values() {
    let mut tree = EditableTree::new();
    let a = tree.create_value_node("t", "a", "A");
    let renamed = tree.create_value_node("t", "z", "A");
    let moved = tree.new_node("t", "a", 1..2, &[], "A");
    let other_value = tree.create_value_node("t", "a", "B");
    let same = tree.create_value_node("t", "a", "A");

    assert!(tree.is_equal(a, &tree, same));
    assert!(!tree.is_equal(a, &tree, renamed));
    assert!(!tree.is_equal(a, &tree, moved));
    assert!(!tree.is_equal(a, &tree, other_value));

    let mut other = EditableTree::new();
    let there = other.create_value_node("t", "a", "A");
    assert!(tree.is_equal(a, &other, there), "across trees");
}

// ─── The read-only tree's share ─────────────────────────────────────

#[test]
fn the_read_only_tree_walks_finds_and_flattens_as_the_editable_one() {
    let result = pattern("expr").exec(TEXT);
    let tree = result.tree().unwrap();
    let root = tree.root();
    let editable = tree.to_mut();
    let edit_root = editable.root().unwrap();
    let ids = |nodes: Vec<cpat_core::NodeRef>| -> Vec<u32> {
        nodes.into_iter().map(|n| editable.id(n)).collect()
    };
    let read_ids = |nodes: Vec<cpat_core::Node<'_, '_>>| -> Vec<u32> {
        nodes.into_iter().map(|n| n.id().index()).collect()
    };

    assert_eq!(read_ids(root.flatten()), ids(editable.flatten(edit_root)));
    assert_eq!(
        root.flatten().iter().map(|n| n.value()).collect::<String>(),
        TEXT
    );
    assert_eq!(
        read_ids(root.find_all_breadth_first(|n| n.name() == "^")),
        ids(editable.find_all_breadth_first(edit_root, |t, n| t.name(n) == "^"))
    );
    assert_eq!(
        read_ids(root.find_all(|n| n.is_leaf())),
        ids(editable.find_all(edit_root, |t, n| t.is_leaf(n)))
    );
    // Depth first, the outermost matching ancestor wins; breadth first, the
    // first match met.
    assert_eq!(root.find(|n| n.name() == "pow").unwrap().value(), "3!^x^2");
    assert_eq!(
        root.find_breadth_first(|n| n.name() == "pow")
            .unwrap()
            .value(),
        "3!^x^2"
    );
    assert_eq!(root.find(|n| n.name() == "variable").unwrap().value(), "x");
    assert_eq!(
        root.find_breadth_first(|n| n.name() == "variable")
            .unwrap()
            .value(),
        "y"
    );

    let mut down = Vec::new();
    assert!(root.walk_down(|n| {
        down.push(n.id().index());
        true
    }));
    let mut edit_down = Vec::new();
    editable.clone().walk_down(edit_root, |t, n| {
        edit_down.push(t.id(n));
        true
    });
    assert_eq!(down, edit_down);

    let mut up = 0;
    assert!(!root.walk_up(|n| {
        up += 1;
        n.name() != "-"
    }));
    // `-`, then its ancestors `neg`, `mul` and `add`; their other children
    // are skipped.
    assert_eq!(up, 4);

    let mut level = 0;
    assert!(!root.walk_breadth_first(|n| {
        level += 1;
        n.name() != "+"
    }));
    // `add`, then its children `mul`, `spaces` and `+`.
    assert_eq!(level, 4);
}

#[test]
fn an_operator_chain_as_deep_as_the_input_walks_prints_and_edits_on_a_small_stack() {
    // A Pratt chain builds a tree one level per operator (§7e), so nothing
    // over a tree may recurse. The test runs on a 256 KiB stack, too small
    // for one native frame per level at this depth. (All of it, grammar
    // included: names are interned per thread.) The depth is modest because
    // v12's JSON and cycle-free values repeat each subtree's text at every
    // level, so their size grows with the square of the depth.
    const N: usize = 5_000;
    let run = || {
        let text = format!("{}1", "-".repeat(N));
        let result = pattern("expr").exec(&text);
        let tree = result.tree().unwrap();

        let json = tree.root().to_json(0);
        assert_eq!(tree.root().flatten().len(), N + 1);
        assert_eq!(tree.root().find_all(|_| true).len(), 2 * N + 1);

        let mut editable = tree.to_mut();
        let root = editable.root().unwrap();
        assert_eq!(editable.to_json(root, 0), json);

        let copy = editable.clone_node(root);
        assert!(editable.is_equal(copy, &editable, root));
        assert_eq!(editable.normalize(copy), N + 1);
        let object = editable.to_cycle_free_object(copy);
        assert_eq!(object.value.len(), N + 1);
        assert_eq!(object, editable.to_cycle_free_object(copy));
        drop(object);

        let mut visitors = Visitors::new();
        assert_eq!(editable.transform(root, &mut visitors), root);
        assert_eq!(editable.find_all(root, |_, _| true).len(), 2 * N + 1);
        editable.compact(root);
        assert_eq!(editable.value(root), text);
    };

    std::thread::Builder::new()
        .stack_size(256 * 1024)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap();
}
