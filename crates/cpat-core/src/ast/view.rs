use std::borrow::Cow;

use crate::tree::{NodeId, Tree};

/// What the walks, the JSON writer and equality read from a tree.
///
/// Both forms implement it, the read-only `Tree` and the `EditableTree`, so
/// each algorithm is written once and the two print and compare alike.
pub(crate) trait AstView {
    type Ref: Copy;

    fn children_of(&self, node: Self::Ref) -> &[Self::Ref];
    fn id_of(&self, node: Self::Ref) -> u32;
    fn type_of(&self, node: Self::Ref) -> &str;
    fn name_of(&self, node: Self::Ref) -> &str;
    /// The value a node has while it has no children (v12's `_value`).
    fn own_value_of(&self, node: Self::Ref) -> &str;
    fn start_of(&self, node: Self::Ref) -> usize;
    fn end_of(&self, node: Self::Ref) -> usize;
}

/// A shared borrow reads the same, so the walks can hand a read-only caller
/// `&mut &Tree` where a mutating caller gets `&mut EditableTree`.
impl<V: AstView + ?Sized> AstView for &V {
    type Ref = V::Ref;

    fn children_of(&self, node: Self::Ref) -> &[Self::Ref] {
        (**self).children_of(node)
    }
    fn id_of(&self, node: Self::Ref) -> u32 {
        (**self).id_of(node)
    }
    fn type_of(&self, node: Self::Ref) -> &str {
        (**self).type_of(node)
    }
    fn name_of(&self, node: Self::Ref) -> &str {
        (**self).name_of(node)
    }
    fn own_value_of(&self, node: Self::Ref) -> &str {
        (**self).own_value_of(node)
    }
    fn start_of(&self, node: Self::Ref) -> usize {
        (**self).start_of(node)
    }
    fn end_of(&self, node: Self::Ref) -> usize {
        (**self).end_of(node)
    }
}

impl AstView for Tree<'_> {
    type Ref = NodeId;

    fn children_of(&self, node: NodeId) -> &[NodeId] {
        self.child_ids(node)
    }
    fn id_of(&self, node: NodeId) -> u32 {
        node.index()
    }
    fn type_of(&self, node: NodeId) -> &str {
        self.node(node).kind().as_str()
    }
    fn name_of(&self, node: NodeId) -> &str {
        self.node(node).name()
    }
    fn own_value_of(&self, node: NodeId) -> &str {
        self.node(node).value()
    }
    fn start_of(&self, node: NodeId) -> usize {
        self.node(node).start()
    }
    fn end_of(&self, node: NodeId) -> usize {
        self.node(node).end()
    }
}

/// v12's `toString()`: a leaf's own value, or its children's joined. That is
/// the leaves' own values in order, gathered with a stack because an
/// operator chain builds a tree as deep as the input is long (§7e).
pub(crate) fn joined_value<V: AstView>(view: &V, node: V::Ref) -> Cow<'_, str> {
    if view.children_of(node).is_empty() {
        return Cow::Borrowed(view.own_value_of(node));
    }

    let mut value = String::new();
    let mut pending = vec![node];
    while let Some(node) = pending.pop() {
        let children = view.children_of(node);
        if children.is_empty() {
            value.push_str(view.own_value_of(node));
        } else {
            pending.extend(children.iter().rev());
        }
    }
    Cow::Owned(value)
}
