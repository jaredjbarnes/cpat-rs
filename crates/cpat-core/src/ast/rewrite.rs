//! The edits that rebuild a subtree: compact, clone and normalize.

use std::borrow::Cow;

use super::editable::{EditableTree, NodeRef};

impl EditableTree<'_> {
    /// Folds the subtree into a leaf whose value is the subtree's text.
    pub fn compact(&mut self, node: NodeRef) {
        if self.is_leaf(node) {
            return;
        }
        let value = self.value(node).into_owned();
        self.remove_all_children(node);
        self.slot_mut(node).value = Cow::Owned(value);
    }

    /// A deep copy, parentless, with new ids. As in v12, children are made
    /// before their parent, so they take the lower ids.
    pub fn clone_node(&mut self, node: NodeRef) -> NodeRef {
        // Each frame: the node being copied, how many children are copied,
        // and where its copies start on `copies`.
        let mut copies: Vec<NodeRef> = Vec::new();
        let mut frames = vec![(node, 0usize, 0usize)];
        loop {
            let (original, next, _) = frames.last_mut().expect("until the root is copied");
            let original = *original;
            if let Some(&child) = self.children(original).get(*next) {
                *next += 1;
                frames.push((child, 0, copies.len()));
                continue;
            }

            let (_, _, at) = frames.pop().expect("checked above");
            let children = copies.split_off(at);
            let slot = self.slot(original);
            let (type_name, name, value) = (
                slot.type_name.clone(),
                slot.name.clone(),
                slot.value.clone(),
            );
            let range = slot.start..slot.end;
            let copy = self.new_node(type_name, name, range, &children, value);
            if frames.is_empty() {
                return copy;
            }
            copies.push(copy);
        }
    }

    /// v12's `normalize()`: renumbers the subtree from its own start.
    pub fn normalize(&mut self, node: NodeRef) -> usize {
        let start = self.start_index(node);
        self.normalize_from(node, start)
    }

    /// v12's `normalize(startIndex)`: lays the leaves end to end from
    /// `start` by the length of their values and gives each node the span
    /// of its leaves. Returns the subtree's length.
    ///
    /// Lengths are bytes, as every position here is (§5a); v12 counts
    /// UTF-16 units. v12 clamps a last index at 0, so an empty node at 0
    /// still ends at 1; that is kept.
    pub fn normalize_from(&mut self, node: NodeRef, start: usize) -> usize {
        struct Frame {
            node: NodeRef,
            start: usize,
            next: usize,
            length: usize,
        }

        let mut frames = vec![Frame {
            node,
            start,
            next: 0,
            length: 0,
        }];
        loop {
            let top = frames.last_mut().expect("until the root returns");
            if let Some(&child) = self.children(top.node).get(top.next) {
                top.next += 1;
                let start = top.start + top.length;
                frames.push(Frame {
                    node: child,
                    start,
                    next: 0,
                    length: 0,
                });
                continue;
            }

            let frame = frames.pop().expect("checked above");
            let slot = self.slot_mut(frame.node);
            let length = if frame.next == 0 {
                slot.value.len()
            } else {
                frame.length
            };
            slot.start = frame.start;
            slot.end = (frame.start + length).max(1);
            match frames.last_mut() {
                Some(parent) => parent.length += length,
                None => return length,
            }
        }
    }
}
