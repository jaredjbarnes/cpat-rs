//! Where the next selector looks, relative to each node selected so far.

use crate::ast::{EditableTree, NodeRef};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Combinator {
    /// `a + b`: the next sibling.
    Adjacent,
    /// `a ~ b`: every later sibling.
    After,
    /// `a > b`: the children.
    DirectChild,
    /// `a b`, and also no combinator at all (v12's `null`): the node and
    /// everything under it, children before their parent.
    Descendant,
}

impl Combinator {
    pub fn named(name: &str) -> Option<Combinator> {
        Some(match name {
            "adjacent" => Combinator::Adjacent,
            "after" => Combinator::After,
            "direct-child" => Combinator::DirectChild,
            "descendant" => Combinator::Descendant,
            _ => return None,
        })
    }

    /// The nodes this combinator reaches from `node` that satisfy `predicate`.
    pub fn select(
        self,
        tree: &EditableTree<'_>,
        node: NodeRef,
        predicate: &dyn Fn(&EditableTree<'_>, NodeRef) -> bool,
    ) -> Vec<NodeRef> {
        match self {
            Combinator::Adjacent => tree
                .next_sibling(node)
                .filter(|&sibling| predicate(tree, sibling))
                .into_iter()
                .collect(),
            Combinator::After => {
                let Some(parent) = tree.parent(node) else {
                    return Vec::new();
                };
                // v12's `indexOf` is -1 for a node its parent doesn't list
                // (an adopted node stays in its old parent's list, §20), and
                // `slice(-1 + 1)` is then every child.
                let from = tree.find_child_index(parent, node).map_or(0, |i| i + 1);
                tree.children(parent)[from..]
                    .iter()
                    .copied()
                    .filter(|&sibling| predicate(tree, sibling))
                    .collect()
            }
            Combinator::DirectChild => tree
                .children(node)
                .iter()
                .copied()
                .filter(|&child| predicate(tree, child))
                .collect(),
            Combinator::Descendant => tree.find_all(node, |tree, n| predicate(tree, n)),
        }
    }
}
