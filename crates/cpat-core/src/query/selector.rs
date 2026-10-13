//! v12's `Selector`: a compiled selector run over nodes of an editable tree.

use std::collections::HashSet;

use super::combinator::Combinator;
use super::error::SelectorError;
use super::grammar::parse_selector;
use super::step::{Step, compile};
use crate::ast::{EditableTree, NodeRef};

/// A CSS-like selector over node names and properties: `name`, `*`,
/// `[property op 'text']` (`=`, `!=`, `^=`, `$=`, `*=`, `>=`, `<=`, `>`, `<`,
/// compared as JS compares them), `name[…]`, the combinators `a b`,
/// `a > b`, `a + b` and `a ~ b`, and `a, b`.
///
/// ```
/// use cpat_core::{EditableTree, Selector};
///
/// let mut tree = EditableTree::new();
/// let one = tree.create_value_node("node", "child", "1");
/// let two = tree.create_value_node("node", "child", "2");
/// let root = tree.create_node("node", "parent", &[one, two]);
///
/// let selector = Selector::new("parent > [value='2']")?;
/// assert_eq!(selector.find(&tree, &[root]), [two]);
/// assert!(Selector::new("parent >").is_err());
/// # Ok::<(), cpat_core::SelectorError>(())
/// ```
#[derive(Clone, Debug)]
pub struct Selector {
    steps: Vec<Step>,
}

impl Selector {
    /// Parses and compiles `selector`. Text the selector grammar rejects is
    /// an error with v12's message.
    pub fn new(selector: &str) -> Result<Selector, SelectorError> {
        let tree = parse_selector(selector)?;
        Ok(Selector {
            steps: compile(&tree)?,
        })
    }

    /// What the selector reaches from `nodes`; with no combinator first,
    /// that is `nodes` and everything under them, children before parents.
    /// A node reached twice is listed twice, as in v12.
    pub fn find(&self, tree: &EditableTree<'_>, nodes: &[NodeRef]) -> Vec<NodeRef> {
        self.run(tree, nodes.to_vec())
    }

    /// Those of `nodes` the selector picks out when run from their root,
    /// in the order the selector reaches them.
    pub fn filter(&self, tree: &EditableTree<'_>, nodes: &[NodeRef]) -> Vec<NodeRef> {
        let Some(selected) = self.run_from_root(tree, nodes) else {
            return Vec::new();
        };
        let wanted: HashSet<NodeRef> = nodes.iter().copied().collect();
        selected
            .into_iter()
            .filter(|n| wanted.contains(n))
            .collect()
    }

    /// Those of `nodes` the selector does not pick out from their root.
    pub fn not(&self, tree: &EditableTree<'_>, nodes: &[NodeRef]) -> Vec<NodeRef> {
        let Some(selected) = self.run_from_root(tree, nodes) else {
            return Vec::new();
        };
        let selected: HashSet<NodeRef> = selected.into_iter().collect();
        nodes
            .iter()
            .copied()
            .filter(|n| !selected.contains(n))
            .collect()
    }

    /// For each of `nodes`, its nearest ancestor the selector picks out from
    /// their root; each ancestor once, in the order first reached.
    pub fn parents(&self, tree: &EditableTree<'_>, nodes: &[NodeRef]) -> Vec<NodeRef> {
        let Some(selected) = self.run_from_root(tree, nodes) else {
            return Vec::new();
        };
        let selected: HashSet<NodeRef> = selected.into_iter().collect();
        unique(
            nodes
                .iter()
                .filter_map(|&n| tree.find_ancestor(n, |_, a| selected.contains(&a))),
        )
    }

    /// v12 runs `filter`, `not` and `parents` from the root of the first
    /// node, and returns nothing for no nodes.
    fn run_from_root(&self, tree: &EditableTree<'_>, nodes: &[NodeRef]) -> Option<Vec<NodeRef>> {
        let first = *nodes.first()?;
        Some(self.run(tree, vec![tree.find_root(first)]))
    }

    fn run(&self, tree: &EditableTree<'_>, mut selected: Vec<NodeRef>) -> Vec<NodeRef> {
        let mut combinator = Combinator::Descendant;
        for step in &self.steps {
            let each = |predicate: &dyn Fn(&EditableTree<'_>, NodeRef) -> bool| -> Vec<NodeRef> {
                selected
                    .iter()
                    .flat_map(|&n| combinator.select(tree, n, predicate))
                    .collect()
            };
            selected = match step {
                Step::WildCard => each(&|_, _| true),
                Step::Or(parts) => unique(parts.iter().flat_map(|part| part.find(tree, &selected))),
                Step::Name(name) => each(&|t, n| t.name(n) == name),
                Step::NameWithAttribute { name, test } => {
                    each(&|t, n| t.name(n) == name && test.matches(t, n))
                }
                Step::Attribute(test) => each(&|t, n| test.matches(t, n)),
                Step::Combinator(next) => {
                    combinator = *next;
                    continue;
                }
            };
        }
        selected
    }
}

/// Each node once, first occurrence first (a JS `Set`'s order).
pub(crate) fn unique(nodes: impl IntoIterator<Item = NodeRef>) -> Vec<NodeRef> {
    let mut seen = HashSet::new();
    nodes.into_iter().filter(|&n| seen.insert(n)).collect()
}
