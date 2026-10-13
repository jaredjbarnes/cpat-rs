//! Walking and searching an `EditableTree`, as v12's `Node` does.

use super::cycle_free::CycleFreeNode;
use super::editable::{EditableTree, NodeRef};
use super::{equality, json, walk};

impl<'t> EditableTree<'t> {
    /// Children before their parent. The visitor may edit the tree; a node's
    /// children are read before the walk enters it. A `false` skips the
    /// rest of that node's siblings, and every ancestor is still visited
    /// (v12). Returns `false` if any visit did.
    pub fn walk_up(
        &mut self,
        node: NodeRef,
        visit: impl FnMut(&mut EditableTree<'t>, NodeRef) -> bool,
    ) -> bool {
        walk::walk_up(self, node, visit)
    }

    /// A parent before its children, read before its visit. The first
    /// `false` ends the walk.
    pub fn walk_down(
        &mut self,
        node: NodeRef,
        visit: impl FnMut(&mut EditableTree<'t>, NodeRef) -> bool,
    ) -> bool {
        walk::walk_down(self, node, visit)
    }

    /// Level by level; a node's children are read after its visit. The first
    /// `false` ends the walk.
    pub fn walk_breadth_first(
        &mut self,
        node: NodeRef,
        visit: impl FnMut(&mut EditableTree<'t>, NodeRef) -> bool,
    ) -> bool {
        walk::walk_breadth_first(self, node, visit)
    }

    /// v12's depth-first `find`: the first match in `walk_up` order, unless
    /// an ancestor of it matches too, in which case the outermost such
    /// ancestor wins (v12 keeps testing ancestors after a match).
    pub fn find(
        &self,
        node: NodeRef,
        mut predicate: impl FnMut(&EditableTree<'t>, NodeRef) -> bool,
    ) -> Option<NodeRef> {
        walk::find(self, node, |tree, n| predicate(tree, n))
    }

    pub fn find_breadth_first(
        &self,
        node: NodeRef,
        mut predicate: impl FnMut(&EditableTree<'t>, NodeRef) -> bool,
    ) -> Option<NodeRef> {
        walk::find_breadth_first(self, node, |tree, n| predicate(tree, n))
    }

    /// Every match, children before their parent.
    pub fn find_all(
        &self,
        node: NodeRef,
        mut predicate: impl FnMut(&EditableTree<'t>, NodeRef) -> bool,
    ) -> Vec<NodeRef> {
        walk::find_all(self, node, |tree, n| predicate(tree, n))
    }

    pub fn find_all_breadth_first(
        &self,
        node: NodeRef,
        mut predicate: impl FnMut(&EditableTree<'t>, NodeRef) -> bool,
    ) -> Vec<NodeRef> {
        walk::find_all_breadth_first(self, node, |tree, n| predicate(tree, n))
    }

    /// The topmost node above this one, or itself.
    pub fn find_root(&self, node: NodeRef) -> NodeRef {
        let mut node = node;
        while let Some(parent) = self.parent(node) {
            node = parent;
        }
        node
    }

    /// The nearest ancestor (not the node itself) that matches.
    pub fn find_ancestor(
        &self,
        node: NodeRef,
        mut predicate: impl FnMut(&EditableTree<'t>, NodeRef) -> bool,
    ) -> Option<NodeRef> {
        let mut parent = self.parent(node);
        while let Some(ancestor) = parent {
            if predicate(self, ancestor) {
                return Some(ancestor);
            }
            parent = self.parent(ancestor);
        }
        None
    }

    /// The leaves, in order.
    pub fn flatten(&self, node: NodeRef) -> Vec<NodeRef> {
        walk::flatten(self, node)
    }

    /// v12's `isEqual`: type, name, indices, value and children alike. `other`
    /// may be this tree.
    pub fn is_equal(&self, node: NodeRef, other: &EditableTree<'_>, other_node: NodeRef) -> bool {
        equality::is_equal(self, node, other, other_node)
    }

    pub fn to_cycle_free_object(&self, node: NodeRef) -> CycleFreeNode {
        CycleFreeNode::build(self, node)
    }

    /// `JSON.stringify(toCycleFreeObject(), null, space)`: keys in v12's
    /// order, ids as strings, `space` 0 for no whitespace.
    pub fn to_json(&self, node: NodeRef, space: usize) -> String {
        json::to_json(self, node, space)
    }
}
