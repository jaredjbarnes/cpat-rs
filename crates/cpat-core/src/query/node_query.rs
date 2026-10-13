//! v12's `Query` and `$`: a selection of nodes in an `EditableTree`, to
//! narrow with selectors and edit in bulk.

use std::rc::Rc;

use super::error::SelectorError;
use super::selector::{Selector, unique};
use crate::ast::{EditableTree, NodeRef};

/// A selection of nodes, and the selection it was narrowed from (`end`).
///
/// It holds handles, not the tree, so several queries over one tree can be
/// kept at once (as v12's are) and each call takes the tree it reads or
/// edits. A query never changes its own selection: edits change the tree,
/// and narrowing returns a new query that remembers this one.
///
/// ```
/// use cpat_core::{EditableTree, Query};
///
/// let mut tree = EditableTree::new();
/// let one = tree.create_value_node("node", "child", "1");
/// let two = tree.create_value_node("node", "child", "2");
/// let root = tree.create_node("node", "parent", &[one, two]);
///
/// let children = Query::select(&tree, root, "child")?;
/// children.last().replace_with(&mut tree, |t, n| {
///     let name = t.name(n).to_owned();
///     t.create_value_node("node", name, "3")
/// });
/// assert_eq!(tree.to_string(root), "13");
/// assert_eq!(children.end().to_array(), [root]);
/// # Ok::<(), cpat_core::SelectorError>(())
/// ```
#[derive(Clone, Debug)]
pub struct Query {
    context: Rc<[NodeRef]>,
    previous: Option<Rc<Query>>,
}

impl Query {
    /// v12's `new Query(context)`: a selection with nothing before it.
    pub fn new(context: Vec<NodeRef>) -> Query {
        Query {
            context: context.into(),
            previous: None,
        }
    }

    /// v12's `$(root)`: just the root.
    pub fn of(root: NodeRef) -> Query {
        Query::new(vec![root])
    }

    /// v12's `$(root, selector)`: what the selector finds from the root, with
    /// `$(root)` before it.
    pub fn select(
        tree: &EditableTree<'_>,
        root: NodeRef,
        selector: &str,
    ) -> Result<Query, SelectorError> {
        Query::of(root).find(tree, selector)
    }

    fn narrowed(&self, context: Vec<NodeRef>) -> Query {
        Query {
            context: context.into(),
            previous: Some(Rc::new(self.clone())),
        }
    }

    pub fn to_array(&self) -> Vec<NodeRef> {
        self.context.to_vec()
    }

    pub fn length(&self) -> usize {
        self.context.len()
    }

    // ─── Edits ──────────────────────────────────────────────────────
    // Each runs over the selection in order and returns this query, as
    // v12's return `this`. As in v12, `append`, `prepend`, `after` and
    // `before` skip a node without a parent (the root, a removed node)
    // without calling the visitor for it.

    /// Adds the visitor's node as the last child of each node.
    pub fn append<'t>(
        &self,
        tree: &mut EditableTree<'t>,
        mut visitor: impl FnMut(&mut EditableTree<'t>, NodeRef) -> NodeRef,
    ) -> &Query {
        for &node in self.context.iter() {
            if tree.parent(node).is_some() {
                let new_node = visitor(tree, node);
                tree.append_child(node, new_node);
            }
        }
        self
    }

    /// Adds the visitor's node as the first child of each node.
    pub fn prepend<'t>(
        &self,
        tree: &mut EditableTree<'t>,
        mut visitor: impl FnMut(&mut EditableTree<'t>, NodeRef) -> NodeRef,
    ) -> &Query {
        for &node in self.context.iter() {
            if tree.parent(node).is_some() {
                let new_node = visitor(tree, node);
                let first = tree.children(node).first().copied();
                tree.insert_before(node, new_node, first);
            }
        }
        self
    }

    /// Puts the visitor's node just after each node.
    pub fn after<'t>(
        &self,
        tree: &mut EditableTree<'t>,
        mut visitor: impl FnMut(&mut EditableTree<'t>, NodeRef) -> NodeRef,
    ) -> &Query {
        for &node in self.context.iter() {
            if let Some(parent) = tree.parent(node) {
                // v12's `indexOf` is -1 for a node its parent doesn't list
                // (adopting leaves it in the old list, §20): it inserts at 0.
                let index = tree.find_child_index(parent, node).map_or(0, |i| i + 1);
                let new_node = visitor(tree, node);
                tree.splice_children(parent, index, 0, &[new_node]);
            }
        }
        self
    }

    /// Puts the visitor's node just before each node.
    pub fn before<'t>(
        &self,
        tree: &mut EditableTree<'t>,
        mut visitor: impl FnMut(&mut EditableTree<'t>, NodeRef) -> NodeRef,
    ) -> &Query {
        for &node in self.context.iter() {
            if let Some(parent) = tree.parent(node) {
                // A missing node is v12's index -1, and `splice(-1, 0, x)`
                // inserts before the last child.
                let index = tree
                    .find_child_index(parent, node)
                    .unwrap_or_else(|| tree.children(parent).len().saturating_sub(1));
                let new_node = visitor(tree, node);
                tree.splice_children(parent, index, 0, &[new_node]);
            }
        }
        self
    }

    /// Puts the visitor's node where each node is. The visitor runs for every
    /// node; one without a parent stays where it is.
    pub fn replace_with<'t>(
        &self,
        tree: &mut EditableTree<'t>,
        mut visitor: impl FnMut(&mut EditableTree<'t>, NodeRef) -> NodeRef,
    ) -> &Query {
        for &node in self.context.iter() {
            let new_node = visitor(tree, node);
            tree.replace_with(node, new_node);
        }
        self
    }

    /// Folds each node into a leaf of its text.
    pub fn compact(&self, tree: &mut EditableTree<'_>) -> &Query {
        for &node in self.context.iter() {
            tree.compact(node);
        }
        self
    }

    /// Replaces each node with a leaf of the same type and name reading
    /// `value`.
    pub fn set_value(&self, tree: &mut EditableTree<'_>, value: &str) -> &Query {
        self.replace_with(tree, |tree, node| {
            let type_name = tree.type_name(node).to_owned();
            let name = tree.name(node).to_owned();
            tree.create_value_node(type_name, name, value.to_owned())
        })
    }

    /// Renumbers the whole tree the first node is in. Like v12's, it returns
    /// nothing to chain on.
    pub fn normalize(&self, tree: &mut EditableTree<'_>) {
        if let Some(&first) = self.context.first() {
            let root = tree.find_root(first);
            tree.normalize(root);
        }
    }

    /// Detaches each node from its parent.
    pub fn remove(&self, tree: &mut EditableTree<'_>) -> &Query {
        for &node in self.context.iter() {
            tree.remove(node);
        }
        self
    }

    // ─── Narrowing ──────────────────────────────────────────────────

    /// `Array.prototype.slice` of the selection: negative indices count from
    /// the end. Unlike every other narrowing, v12's `slice` forgets the
    /// query it came from, so `end()` on it returns itself.
    pub fn slice(&self, start: isize, end: Option<isize>) -> Query {
        Query::new(js_slice(&self.context, start, end))
    }

    /// The selected nodes the selector picks out from their root.
    pub fn filter(&self, tree: &EditableTree<'_>, selector: &str) -> Result<Query, SelectorError> {
        let selector = Selector::new(selector)?;
        Ok(self.narrowed(selector.filter(tree, &self.context)))
    }

    /// What the selector finds from the selected nodes (themselves included).
    pub fn find(&self, tree: &EditableTree<'_>, selector: &str) -> Result<Query, SelectorError> {
        let selector = Selector::new(selector)?;
        Ok(self.narrowed(selector.find(tree, &self.context)))
    }

    /// The selected nodes the selector does not pick out from their root.
    pub fn not(&self, tree: &EditableTree<'_>, selector: &str) -> Result<Query, SelectorError> {
        let selector = Selector::new(selector)?;
        Ok(self.narrowed(selector.not(tree, &self.context)))
    }

    /// Each selected node's parent, once each.
    pub fn parent(&self, tree: &EditableTree<'_>) -> Query {
        self.narrowed(unique(self.context.iter().filter_map(|&n| tree.parent(n))))
    }

    /// Each selected node's nearest ancestor the selector picks out.
    pub fn parents(&self, tree: &EditableTree<'_>, selector: &str) -> Result<Query, SelectorError> {
        let selector = Selector::new(selector)?;
        Ok(self.narrowed(selector.parents(tree, &self.context)))
    }

    pub fn first(&self) -> Query {
        self.narrowed(js_slice(&self.context, 0, Some(1)))
    }

    pub fn last(&self) -> Query {
        self.narrowed(js_slice(&self.context, -1, None))
    }

    /// The query this one was narrowed from, or this one if none.
    pub fn end(&self) -> Query {
        match &self.previous {
            Some(previous) => Query::clone(previous),
            None => self.clone(),
        }
    }
}

fn js_slice(items: &[NodeRef], start: isize, end: Option<isize>) -> Vec<NodeRef> {
    let len = items.len() as isize;
    let clamp = |index: isize| {
        if index < 0 {
            (len + index).max(0)
        } else {
            index.min(len)
        }
    };
    let (start, end) = (clamp(start), end.map_or(len, clamp));
    if start >= end {
        return Vec::new();
    }
    items[start as usize..end as usize].to_vec()
}
