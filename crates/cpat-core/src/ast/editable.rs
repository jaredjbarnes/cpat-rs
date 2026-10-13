use std::borrow::Cow;
use std::ops::Range;

use super::view::{AstView, joined_value};

/// A handle to one node of an `EditableTree`: v12's node identity (`toBe`).
///
/// It is an index, so it is only meaningful with the tree that made it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeRef(pub(crate) u32);

#[derive(Clone, Debug)]
pub(crate) struct Slot<'t> {
    pub id: u32,
    pub type_name: Cow<'static, str>,
    pub name: Cow<'static, str>,
    /// The value while the node has no children. A parse leaf borrows its
    /// slice of the input; only `compact` and new nodes own text.
    pub value: Cow<'t, str>,
    pub start: usize,
    pub end: usize,
    pub parent: Option<NodeRef>,
    pub children: Vec<NodeRef>,
}

/// The editable form of a tree (§7b): v12's `Node` API over an arena.
///
/// Every node lives in the arena for the tree's life. A node removed from its
/// parent stays there, parentless, as a v12 node does while something holds
/// it, and can be put back. Methods take the `NodeRef` they act on, as v12's
/// take `this`. Positions are byte offsets, half-open (§5).
#[derive(Clone, Debug, Default)]
pub struct EditableTree<'t> {
    pub(crate) slots: Vec<Slot<'t>>,
    pub(crate) root: Option<NodeRef>,
    /// The id the next new node gets. v12 numbers nodes from one global
    /// counter; here each tree counts on from the ids it already holds.
    pub(crate) next_id: u32,
}

impl<'t> EditableTree<'t> {
    /// An empty tree to build nodes in with `create_node` and friends.
    pub fn new() -> EditableTree<'t> {
        EditableTree::default()
    }

    /// The root of the parse tree this was made from (`Tree::to_mut`); `None`
    /// for a tree built from nothing. Edits never move it: a root has no
    /// parent, so nothing replaces it.
    pub fn root(&self) -> Option<NodeRef> {
        self.root
    }

    pub(crate) fn slot(&self, node: NodeRef) -> &Slot<'t> {
        &self.slots[node.0 as usize]
    }

    pub(crate) fn slot_mut(&mut self, node: NodeRef) -> &mut Slot<'t> {
        &mut self.slots[node.0 as usize]
    }

    // ─── Making nodes ───────────────────────────────────────────────

    /// v12's constructor: a node over `range` that adopts `children` (they
    /// are not taken out of any other parent's list, as in v12). `value` is
    /// what the node reads as once it has no children.
    pub fn new_node(
        &mut self,
        type_name: impl Into<Cow<'static, str>>,
        name: impl Into<Cow<'static, str>>,
        range: Range<usize>,
        children: &[NodeRef],
        value: impl Into<Cow<'t, str>>,
    ) -> NodeRef {
        let node = NodeRef(self.slots.len() as u32);
        let id = self.next_id;
        self.next_id += 1;
        self.slots.push(Slot {
            id,
            type_name: type_name.into(),
            name: name.into(),
            value: value.into(),
            start: range.start,
            end: range.end,
            parent: None,
            children: children.to_vec(),
        });
        for &child in children {
            self.slot_mut(child).parent = Some(node);
        }
        node
    }

    /// `Node.createValueNode`: a leaf at v12's default indices 0..=0.
    pub fn create_value_node(
        &mut self,
        type_name: impl Into<Cow<'static, str>>,
        name: impl Into<Cow<'static, str>>,
        value: impl Into<Cow<'t, str>>,
    ) -> NodeRef {
        self.new_node(type_name, name, 0..1, &[], value)
    }

    /// `Node.createNode`: a parent at 0..=0 whose own value is its children's
    /// joined, taken now.
    pub fn create_node(
        &mut self,
        type_name: impl Into<Cow<'static, str>>,
        name: impl Into<Cow<'static, str>>,
        children: &[NodeRef],
    ) -> NodeRef {
        let value: String = children.iter().map(|&c| self.value(c)).collect();
        self.new_node(type_name, name, 0..1, children, value)
    }

    // ─── Reads ──────────────────────────────────────────────────────

    /// v12 ids are strings of a counter; this is that counter.
    pub fn id(&self, node: NodeRef) -> u32 {
        self.slot(node).id
    }

    /// v12's `type`.
    pub fn type_name(&self, node: NodeRef) -> &str {
        &self.slot(node).type_name
    }

    pub fn name(&self, node: NodeRef) -> &str {
        &self.slot(node).name
    }

    /// A leaf's own value, or the children's values joined.
    pub fn value(&self, node: NodeRef) -> Cow<'_, str> {
        joined_value(self, node)
    }

    /// v12's `toString()`, the same as `value`.
    pub fn to_string(&self, node: NodeRef) -> String {
        self.value(node).into_owned()
    }

    pub fn first_index(&self, node: NodeRef) -> usize {
        self.slot(node).start
    }

    /// Inclusive, as v12's: one before `end_index`, so `-1` for an empty node
    /// at the start of the text.
    pub fn last_index(&self, node: NodeRef) -> isize {
        self.slot(node).end as isize - 1
    }

    pub fn start_index(&self, node: NodeRef) -> usize {
        self.slot(node).start
    }

    /// Half-open: just past the node.
    pub fn end_index(&self, node: NodeRef) -> usize {
        self.slot(node).end
    }

    pub fn parent(&self, node: NodeRef) -> Option<NodeRef> {
        self.slot(node).parent
    }

    pub fn children(&self, node: NodeRef) -> &[NodeRef] {
        &self.slot(node).children
    }

    pub fn has_children(&self, node: NodeRef) -> bool {
        !self.children(node).is_empty()
    }

    pub fn is_leaf(&self, node: NodeRef) -> bool {
        !self.has_children(node)
    }
}

impl AstView for EditableTree<'_> {
    type Ref = NodeRef;

    fn children_of(&self, node: NodeRef) -> &[NodeRef] {
        &self.slot(node).children
    }
    fn id_of(&self, node: NodeRef) -> u32 {
        self.slot(node).id
    }
    fn type_of(&self, node: NodeRef) -> &str {
        &self.slot(node).type_name
    }
    fn name_of(&self, node: NodeRef) -> &str {
        &self.slot(node).name
    }
    fn own_value_of(&self, node: NodeRef) -> &str {
        &self.slot(node).value
    }
    fn start_of(&self, node: NodeRef) -> usize {
        self.slot(node).start
    }
    fn end_of(&self, node: NodeRef) -> usize {
        self.slot(node).end
    }
}
