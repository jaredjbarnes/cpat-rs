//! v12's child edits. Each keeps v12's parent bookkeeping exactly, including
//! its quirks: adopting a node does not take it out of its old parent's list,
//! and `insert_before` adopts the new node even when the reference node is
//! not a child.

use super::editable::{EditableTree, NodeRef};

impl EditableTree<'_> {
    pub fn find_child_index(&self, parent: NodeRef, child: NodeRef) -> Option<usize> {
        self.children(parent).iter().position(|&c| c == child)
    }

    pub fn remove_child(&mut self, parent: NodeRef, child: NodeRef) {
        if let Some(index) = self.find_child_index(parent, child) {
            self.slot_mut(parent).children.remove(index);
            self.slot_mut(child).parent = None;
        }
    }

    /// `Array.prototype.splice` on the children: `index` and `delete_count`
    /// are clamped to the list. Returns the removed children, now parentless.
    pub fn splice_children(
        &mut self,
        parent: NodeRef,
        index: usize,
        delete_count: usize,
        items: &[NodeRef],
    ) -> Vec<NodeRef> {
        let children = &mut self.slot_mut(parent).children;
        let index = index.min(children.len());
        let end = index + delete_count.min(children.len() - index);
        let removed: Vec<NodeRef> = children.splice(index..end, items.iter().copied()).collect();

        for &node in &removed {
            self.slot_mut(node).parent = None;
        }
        for &node in items {
            self.slot_mut(node).parent = Some(parent);
        }
        removed
    }

    pub fn remove_all_children(&mut self, parent: NodeRef) {
        let len = self.children(parent).len();
        self.splice_children(parent, 0, len, &[]);
    }

    /// Puts `new_node` where `reference` is. Nothing happens if `reference`
    /// is not a child.
    pub fn replace_child(&mut self, parent: NodeRef, new_node: NodeRef, reference: NodeRef) {
        if let Some(index) = self.find_child_index(parent, reference) {
            self.splice_children(parent, index, 1, &[new_node]);
            self.slot_mut(new_node).parent = Some(parent);
            self.slot_mut(reference).parent = None;
        }
    }

    /// Puts `new_node` where `node` is in its parent. A root stays put.
    pub fn replace_with(&mut self, node: NodeRef, new_node: NodeRef) {
        if let Some(parent) = self.parent(node) {
            self.replace_child(parent, new_node, node);
        }
    }

    /// Before `reference`, or last when it is `None`.
    pub fn insert_before(
        &mut self,
        parent: NodeRef,
        new_node: NodeRef,
        reference: Option<NodeRef>,
    ) {
        self.slot_mut(new_node).parent = Some(parent);
        let Some(reference) = reference else {
            self.slot_mut(parent).children.push(new_node);
            return;
        };
        if let Some(index) = self.find_child_index(parent, reference) {
            self.slot_mut(parent).children.insert(index, new_node);
        }
    }

    pub fn append_child(&mut self, parent: NodeRef, new_node: NodeRef) {
        self.append(parent, &[new_node]);
    }

    pub fn append(&mut self, parent: NodeRef, nodes: &[NodeRef]) {
        for &node in nodes {
            self.slot_mut(node).parent = Some(parent);
            self.slot_mut(parent).children.push(node);
        }
    }

    /// Detaches the node from its parent. It stays in the arena.
    pub fn remove(&mut self, node: NodeRef) {
        if let Some(parent) = self.parent(node) {
            self.remove_child(parent, node);
        }
    }

    pub fn next_sibling(&self, node: NodeRef) -> Option<NodeRef> {
        let siblings = self.children(self.parent(node)?);
        let index = siblings.iter().position(|&c| c == node)?;
        siblings.get(index + 1).copied()
    }

    pub fn previous_sibling(&self, node: NodeRef) -> Option<NodeRef> {
        let siblings = self.children(self.parent(node)?);
        let index = siblings.iter().position(|&c| c == node)?;
        index.checked_sub(1).map(|i| siblings[i])
    }
}
