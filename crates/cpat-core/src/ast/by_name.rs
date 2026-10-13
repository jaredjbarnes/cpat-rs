//! v12's `compact(node, names)` and `remove(node, names)`.

use super::editable::{EditableTree, NodeRef};

/// Compacts every node named in `names`, breadth first, so a compacted
/// node's subtree is never visited. Returns `node`.
pub fn compact(tree: &mut EditableTree<'_>, node: NodeRef, names: &[&str]) -> NodeRef {
    tree.walk_breadth_first(node, |tree, n| {
        if names.contains(&tree.name(n)) {
            tree.compact(n);
        }
        true
    });
    node
}

/// Detaches every node named in `names` from its parent, breadth first.
/// `node` itself has no parent to leave, so it stays. Returns `node`.
pub fn remove(tree: &mut EditableTree<'_>, node: NodeRef, names: &[&str]) -> NodeRef {
    tree.walk_breadth_first(node, |tree, n| {
        if names.contains(&tree.name(n)) {
            tree.remove(n);
        }
        true
    });
    node
}
