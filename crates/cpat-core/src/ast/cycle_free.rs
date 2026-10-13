use std::fmt;

use super::view::{AstView, joined_value};

/// v12's `CycleFreeNode`: a node and its subtree as plain owned data, with
/// no parent links. Building, dropping and comparing it use a stack, not
/// the native one (§7e), so a deep expression tree can't overflow.
pub struct CycleFreeNode {
    pub id: String,
    /// v12's `type`.
    pub type_name: String,
    pub name: String,
    pub value: String,
    pub start_index: usize,
    pub end_index: usize,
    pub children: Vec<CycleFreeNode>,
}

impl CycleFreeNode {
    pub(crate) fn build<V: AstView>(view: &V, root: V::Ref) -> CycleFreeNode {
        let leaf = |node: V::Ref| CycleFreeNode {
            id: view.id_of(node).to_string(),
            type_name: view.type_of(node).to_owned(),
            name: view.name_of(node).to_owned(),
            value: joined_value(view, node).into_owned(),
            start_index: view.start_of(node),
            end_index: view.end_of(node),
            children: Vec::new(),
        };

        // Children are finished before their parent: each frame is a node,
        // how many of its children are done, and where its finished
        // children start on `done`.
        let mut done: Vec<CycleFreeNode> = Vec::new();
        let mut frames = vec![(root, 0usize, 0usize)];
        loop {
            let (node, next, _) = frames.last_mut().expect("until the root is built");
            let children = view.children_of(*node);
            if *next < children.len() {
                let child = children[*next];
                *next += 1;
                let at = done.len();
                frames.push((child, 0, at));
                continue;
            }

            let (node, _, at) = frames.pop().expect("checked above");
            let mut built = leaf(node);
            built.children = done.split_off(at);
            if frames.is_empty() {
                return built;
            }
            done.push(built);
        }
    }

    fn fields_eq(&self, other: &CycleFreeNode) -> bool {
        self.id == other.id
            && self.type_name == other.type_name
            && self.name == other.name
            && self.value == other.value
            && self.start_index == other.start_index
            && self.end_index == other.end_index
            && self.children.len() == other.children.len()
    }
}

impl PartialEq for CycleFreeNode {
    fn eq(&self, other: &CycleFreeNode) -> bool {
        let mut pending = vec![(self, other)];
        while let Some((a, b)) = pending.pop() {
            if !a.fields_eq(b) {
                return false;
            }
            pending.extend(a.children.iter().zip(&b.children));
        }
        true
    }
}

impl Eq for CycleFreeNode {}

/// The default drop recurses once per level.
impl Drop for CycleFreeNode {
    fn drop(&mut self) {
        let mut pending = std::mem::take(&mut self.children);
        while let Some(mut node) = pending.pop() {
            pending.append(&mut node.children);
        }
    }
}

/// Shallow, like `Node`'s: printing the subtree would recurse.
impl fmt::Debug for CycleFreeNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CycleFreeNode")
            .field("id", &self.id)
            .field("type_name", &self.type_name)
            .field("name", &self.name)
            .field("value", &self.value)
            .field("range", &(self.start_index..self.end_index))
            .field("children", &self.children.len())
            .finish()
    }
}
