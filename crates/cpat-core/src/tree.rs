use crate::name::Name;

/// What produced a node. A node's kind is fixed by its pattern, so it is a
/// closed set rather than a string.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Literal,
    Regex,
    Sequence,
    InfiniteRepeat,
    FiniteRepeat,
    Custom,
}

/// Index of a node in its tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(u32);

const NO_PARENT: u32 = u32::MAX;

#[derive(Clone, Copy)]
struct NodeData {
    kind: NodeKind,
    name: Name,
    start: u32,
    end: u32,
    parent: u32,
    children_at: u32,
    children_len: u32,
}

/// The arena nodes are written into while a parse runs.
///
/// It only grows. Nodes made by alternatives that later failed stay in it as
/// garbage until the parse ends; the finished `Tree` reaches only what the root
/// reaches. Truncating on backtrack is tempting but unsafe while history may
/// refer to a node, and history stores copies for exactly that reason.
#[derive(Default)]
pub(crate) struct TreeBuilder {
    nodes: Vec<NodeData>,
    child_ids: Vec<NodeId>,
}

impl TreeBuilder {
    pub fn leaf(&mut self, kind: NodeKind, name: Name, start: usize, end: usize) -> NodeId {
        self.push(kind, name, start, end, &[])
    }

    pub fn branch(
        &mut self,
        kind: NodeKind,
        name: Name,
        start: usize,
        end: usize,
        children: &[NodeId],
    ) -> NodeId {
        self.push(kind, name, start, end, children)
    }

    pub fn start(&self, id: NodeId) -> usize {
        self.nodes[id.0 as usize].start as usize
    }

    pub fn end(&self, id: NodeId) -> usize {
        self.nodes[id.0 as usize].end as usize
    }

    pub fn finish<'t>(self, text: &'t str, root: NodeId) -> Tree<'t> {
        Tree {
            text,
            nodes: self.nodes,
            child_ids: self.child_ids,
            root,
        }
    }

    fn push(
        &mut self,
        kind: NodeKind,
        name: Name,
        start: usize,
        end: usize,
        children: &[NodeId],
    ) -> NodeId {
        let id = self.nodes.len() as u32;

        // A parent is always made after its children, so this is where their
        // parent link gets filled in. Nothing ever re-parents a node.
        for child in children {
            self.nodes[child.0 as usize].parent = id;
        }

        self.nodes.push(NodeData {
            kind,
            name,
            start: start as u32,
            end: end as u32,
            parent: NO_PARENT,
            children_at: self.child_ids.len() as u32,
            children_len: children.len() as u32,
        });
        self.child_ids.extend_from_slice(children);

        NodeId(id)
    }
}

/// A finished, read-only parse tree. Values are slices of the input text.
pub struct Tree<'t> {
    text: &'t str,
    nodes: Vec<NodeData>,
    child_ids: Vec<NodeId>,
    root: NodeId,
}

impl<'t> Tree<'t> {
    pub fn root(&self) -> Node<'_, 't> {
        self.node(self.root)
    }

    pub fn node(&self, id: NodeId) -> Node<'_, 't> {
        Node { tree: self, id }
    }

    pub fn text(&self) -> &'t str {
        self.text
    }
}

/// A view of one node. Cheap to copy; it borrows the tree.
#[derive(Clone, Copy)]
pub struct Node<'a, 't> {
    tree: &'a Tree<'t>,
    id: NodeId,
}

impl<'a, 't> Node<'a, 't> {
    fn data(&self) -> &'a NodeData {
        &self.tree.nodes[self.id.0 as usize]
    }

    pub fn id(&self) -> NodeId {
        self.id
    }

    pub fn kind(&self) -> NodeKind {
        self.data().kind
    }

    pub fn name(&self) -> &'static str {
        self.data().name.as_str()
    }

    /// Byte offset of the first byte the node covers.
    pub fn start(&self) -> usize {
        self.data().start as usize
    }

    /// Byte offset just past the node (half-open).
    pub fn end(&self) -> usize {
        self.data().end as usize
    }

    pub fn value(&self) -> &'t str {
        &self.tree.text[self.start()..self.end()]
    }

    pub fn parent(&self) -> Option<Node<'a, 't>> {
        let parent = self.data().parent;
        (parent != NO_PARENT).then(|| self.tree.node(NodeId(parent)))
    }

    pub fn children(&self) -> impl ExactSizeIterator<Item = Node<'a, 't>> + 'a {
        let data = self.data();
        let at = data.children_at as usize;
        let ids = &self.tree.child_ids[at..at + data.children_len as usize];
        let tree = self.tree;
        ids.iter().map(move |&id| tree.node(id))
    }

    pub fn is_leaf(&self) -> bool {
        self.data().children_len == 0
    }

    /// The first node, depth first with children before their parent, that
    /// satisfies `predicate`. The node itself is included.
    pub fn find(&self, predicate: impl Fn(&Node<'a, 't>) -> bool) -> Option<Node<'a, 't>> {
        self.find_in(&predicate)
    }

    /// Every node, in the same order as `find`, that satisfies `predicate`.
    pub fn find_all(&self, predicate: impl Fn(&Node<'a, 't>) -> bool) -> Vec<Node<'a, 't>> {
        let mut found = Vec::new();
        self.collect(&predicate, &mut found);
        found
    }

    fn find_in(&self, predicate: &dyn Fn(&Node<'a, 't>) -> bool) -> Option<Node<'a, 't>> {
        self.children()
            .find_map(|child| child.find_in(predicate))
            .or_else(|| predicate(self).then_some(*self))
    }

    fn collect(&self, predicate: &dyn Fn(&Node<'a, 't>) -> bool, found: &mut Vec<Node<'a, 't>>) {
        for child in self.children() {
            child.collect(predicate, found);
        }
        if predicate(self) {
            found.push(*self);
        }
    }
}

impl std::fmt::Debug for Node<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("Node");
        s.field("kind", &self.kind())
            .field("name", &self.name())
            .field("range", &(self.start()..self.end()))
            .field("value", &self.value());
        if !self.is_leaf() {
            s.field("children", &self.children().collect::<Vec<_>>());
        }
        s.finish()
    }
}
