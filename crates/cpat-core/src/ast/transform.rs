use std::collections::HashMap;

use super::editable::{EditableTree, NodeRef};

type Visit<'v, 't> = Box<dyn FnMut(&mut EditableTree<'t>, NodeRef) -> NodeRef + 'v>;

/// The visitors for `transform`, by node name. A name with no visitor keeps
/// its node.
#[derive(Default)]
pub struct Visitors<'v, 't> {
    by_name: HashMap<String, Visit<'v, 't>>,
}

impl<'v, 't> Visitors<'v, 't> {
    pub fn new() -> Visitors<'v, 't> {
        Visitors {
            by_name: HashMap::new(),
        }
    }

    /// Visits every node named `name`: it gets the node, its children
    /// already transformed, and returns what takes the node's place.
    pub fn on(
        mut self,
        name: impl Into<String>,
        visit: impl FnMut(&mut EditableTree<'t>, NodeRef) -> NodeRef + 'v,
    ) -> Visitors<'v, 't> {
        self.by_name.insert(name.into(), Box::new(visit));
        self
    }
}

impl<'t> EditableTree<'t> {
    /// v12's `transform`: children first, each replaced by what its visitor
    /// returned, then the node's own visitor. Returns what replaces `node`.
    ///
    /// As in v12, a node's children are taken off it and the transformed
    /// ones appended, even when nothing changed.
    pub fn transform(&mut self, node: NodeRef, visitors: &mut Visitors<'_, 't>) -> NodeRef {
        struct Frame {
            node: NodeRef,
            // The node's children when it was entered, in `snapshot[at..]`,
            // and its transformed children so far, in `results[results_at..]`.
            at: usize,
            next: usize,
            results_at: usize,
        }

        let mut snapshot: Vec<NodeRef> = Vec::new();
        let mut results: Vec<NodeRef> = Vec::new();
        let enter =
            |tree: &EditableTree<'t>, node, snapshot: &mut Vec<NodeRef>, results: &Vec<NodeRef>| {
                let at = snapshot.len();
                snapshot.extend_from_slice(tree.children(node));
                Frame {
                    node,
                    at,
                    next: 0,
                    results_at: results.len(),
                }
            };

        let mut frames = vec![enter(self, node, &mut snapshot, &results)];
        loop {
            let top = frames.last_mut().expect("until the root returns");
            let len = snapshot.len() - top.at;
            // Only the top frame's snapshot is ever past its children, as
            // frames end in stack order.
            if top.next < len {
                let child = snapshot[top.at + top.next];
                top.next += 1;
                let frame = enter(self, child, &mut snapshot, &results);
                frames.push(frame);
                continue;
            }

            let frame = frames.pop().expect("checked above");
            snapshot.truncate(frame.at);
            let children = results.split_off(frame.results_at);
            self.remove_all_children(frame.node);
            self.append(frame.node, &children);

            let visited = match visitors.by_name.get_mut(self.name(frame.node)) {
                Some(visit) => visit(self, frame.node),
                None => frame.node,
            };
            if frames.is_empty() {
                return visited;
            }
            results.push(visited);
        }
    }
}
