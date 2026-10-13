use std::borrow::Cow;

use super::editable::{EditableTree, NodeRef, Slot};
use crate::tree::{NodeId, Tree};

impl<'t> Tree<'t> {
    /// The editable form (§7b): a copy of the nodes the root reaches, with
    /// their ids. Leaves keep borrowing their slice of the input; nothing is
    /// copied until an edit needs it.
    ///
    /// ```
    /// use cpat_core::ast::{compact, remove};
    /// use cpat_core::{EditableTree, Visitors};
    ///
    /// let mut patterns = cpat_core::patterns!(r#"
    ///     space = /\s+/
    ///     number = /\d+/
    ///     group = "(" + space? + expr + space? + ")"
    ///     add = expr + space? + "+" + space? + expr
    ///     expr = add | group | number
    /// "#)?;
    /// let result = patterns.get_mut("expr").unwrap().exec("(1 + 2) + 3");
    /// let mut tree = result.tree().unwrap().to_mut();
    /// let root = tree.root().unwrap();
    ///
    /// compact(&mut tree, root, &["group"]);
    /// remove(&mut tree, root, &["space"]);
    /// let mut visitors = Visitors::new().on("number", |t: &mut EditableTree<'_>, n| {
    ///     let value = t.to_string(n);
    ///     t.create_value_node("number", "num", value)
    /// });
    /// let root = tree.transform(root, &mut visitors);
    ///
    /// assert_eq!(tree.value(root), "(1 + 2)+3");
    /// assert_eq!(tree.children(root).len(), 3);
    /// # Ok::<(), cpat_core::GrammarError>(())
    /// ```
    pub fn to_mut(&self) -> EditableTree<'t> {
        let mut editable = EditableTree {
            slots: Vec::new(),
            root: Some(NodeRef(0)),
            next_id: self.arena_len() as u32,
        };

        // Parents first: each popped node gets the next slot and joins its
        // parent's list. Children are pushed in reverse, so siblings arrive
        // in order.
        let mut pending: Vec<(NodeId, Option<NodeRef>)> = vec![(self.root().id(), None)];
        while let Some((id, parent)) = pending.pop() {
            let node = self.node(id);
            let at = NodeRef(editable.slots.len() as u32);
            editable.slots.push(Slot {
                id: id.index(),
                type_name: Cow::Borrowed(node.kind().as_str()),
                name: Cow::Borrowed(node.name()),
                // A parent reads its value from its children. v12's parse
                // nodes keep "" as their own, so a parent stripped of all its
                // children reads as "" there too.
                value: Cow::Borrowed(if node.is_leaf() { node.value() } else { "" }),
                start: node.start(),
                end: node.end(),
                parent,
                children: Vec::with_capacity(node.children().len()),
            });
            if let Some(parent) = parent {
                editable.slot_mut(parent).children.push(at);
            }
            pending.extend(node.children().rev().map(|child| (child.id(), Some(at))));
        }

        editable
    }
}
