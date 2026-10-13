//! The part of v12's `Node` API that doesn't edit, on the read-only tree, so
//! reading a tree never needs its editable form.

use super::view::AstView;
use super::{json, walk};
use crate::tree::{Node, Tree};

impl<'a, 't> Node<'a, 't> {
    /// Children before their parent. A `false` skips the rest of that node's
    /// siblings, and every ancestor is still visited (v12). Returns `false`
    /// if any visit did.
    pub fn walk_up(&self, mut visit: impl FnMut(Node<'a, 't>) -> bool) -> bool {
        let tree = self.tree();
        walk::walk_up(&mut &*tree, self.id(), |_, id| visit(tree.node(id)))
    }

    /// A parent before its children. The first `false` ends the walk.
    pub fn walk_down(&self, mut visit: impl FnMut(Node<'a, 't>) -> bool) -> bool {
        let tree = self.tree();
        walk::walk_down(&mut &*tree, self.id(), |_, id| visit(tree.node(id)))
    }

    /// Level by level. The first `false` ends the walk.
    pub fn walk_breadth_first(&self, mut visit: impl FnMut(Node<'a, 't>) -> bool) -> bool {
        let tree = self.tree();
        walk::walk_breadth_first(&mut &*tree, self.id(), |_, id| visit(tree.node(id)))
    }

    /// v12's depth-first `find`: the first match in `walk_up` order, unless
    /// an ancestor of it matches too, in which case the outermost such
    /// ancestor wins (v12 keeps testing ancestors after a match).
    pub fn find(&self, mut predicate: impl FnMut(&Node<'a, 't>) -> bool) -> Option<Node<'a, 't>> {
        let tree = self.tree();
        walk::find(tree, self.id(), |_, id| predicate(&tree.node(id))).map(|id| tree.node(id))
    }

    pub fn find_breadth_first(
        &self,
        mut predicate: impl FnMut(&Node<'a, 't>) -> bool,
    ) -> Option<Node<'a, 't>> {
        let tree = self.tree();
        walk::find_breadth_first(tree, self.id(), |_, id| predicate(&tree.node(id)))
            .map(|id| tree.node(id))
    }

    /// Every match, children before their parent.
    pub fn find_all(&self, mut predicate: impl FnMut(&Node<'a, 't>) -> bool) -> Vec<Node<'a, 't>> {
        let tree = self.tree();
        nodes(
            tree,
            walk::find_all(tree, self.id(), |_, id| predicate(&tree.node(id))),
        )
    }

    pub fn find_all_breadth_first(
        &self,
        mut predicate: impl FnMut(&Node<'a, 't>) -> bool,
    ) -> Vec<Node<'a, 't>> {
        let tree = self.tree();
        nodes(
            tree,
            walk::find_all_breadth_first(tree, self.id(), |_, id| predicate(&tree.node(id))),
        )
    }

    /// The leaves, in order.
    pub fn flatten(&self) -> Vec<Node<'a, 't>> {
        let tree = self.tree();
        nodes(tree, walk::flatten(tree, self.id()))
    }

    /// The same JSON `to_mut()` then `to_json` gives, without the copy. A
    /// value is v12's: the leaves' text joined.
    pub fn to_json(&self, space: usize) -> String {
        json::to_json(self.tree(), self.id(), space)
    }
}

fn nodes<'a, 't>(tree: &'a Tree<'t>, ids: Vec<<Tree<'t> as AstView>::Ref>) -> Vec<Node<'a, 't>> {
    ids.into_iter().map(|id| tree.node(id)).collect()
}
