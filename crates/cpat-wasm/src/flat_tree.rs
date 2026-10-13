use cpat_core::{Node, NodeKind};

use crate::utf16::Utf16Offsets;

/// Fields per node in `nodes`.
pub const NODE_STRIDE: usize = 7;
pub(crate) const NONE: u32 = u32::MAX;

/// A tree laid out for JS (see `ExecResult`): `nodes`, `children` and the
/// interned `names`, positions in UTF-16 offsets.
#[derive(Default)]
pub(crate) struct FlatTree {
    pub(crate) nodes: Vec<u32>,
    pub(crate) children: Vec<u32>,
    pub(crate) names: Vec<String>,
}

impl FlatTree {
    pub(crate) fn of(root: Option<Node<'_, '_>>, offsets: &Utf16Offsets) -> FlatTree {
        let mut flat = Flattener {
            offsets,
            nodes: Vec::new(),
            children: Vec::new(),
            names: Vec::new(),
        };
        if let Some(root) = root {
            flat.add_tree(root);
        }
        FlatTree {
            nodes: flat.nodes,
            children: flat.children,
            names: flat.names,
        }
    }
}

struct Flattener<'o> {
    offsets: &'o Utf16Offsets,
    nodes: Vec<u32>,
    children: Vec<u32>,
    names: Vec<String>,
}

impl Flattener<'_> {
    /// Writes `root` and its subtree, depth first with each parent before
    /// its children (so the root is node 0). A node's children list is
    /// reserved when the node is written and each slot is filled as that
    /// child is written, so each list stays contiguous.
    ///
    /// The walk keeps its own stack instead of recursing: an operator chain
    /// makes a tree as deep as the input is long, and a recursive walk ran
    /// out of wasm stack and threw instead of returning the tree.
    fn add_tree(&mut self, root: Node<'_, '_>) {
        // Each entry is a node still to write and the children slot (in
        // its parent's list) that gets its index.
        let mut pending = vec![(root, NONE, None::<usize>)];

        while let Some((node, parent, slot)) = pending.pop() {
            let index = (self.nodes.len() / NODE_STRIDE) as u32;
            if let Some(slot) = slot {
                self.children[slot] = index;
            }

            let name = self.name_index(node.name());
            let children_at = self.children.len();
            let children_len = node.children().len();
            self.children.resize(children_at + children_len, 0);

            self.nodes.extend_from_slice(&[
                kind_code(node.kind()),
                name,
                self.offsets.of(node.start()),
                self.offsets.of(node.end()),
                parent,
                children_at as u32,
                children_len as u32,
            ]);

            // Reversed, so the first child is written next.
            for (i, child) in node.children().enumerate().rev() {
                pending.push((child, index, Some(children_at + i)));
            }
        }
    }

    fn name_index(&mut self, name: &str) -> u32 {
        match self.names.iter().position(|n| n == name) {
            Some(i) => i as u32,
            None => {
                self.names.push(name.to_string());
                (self.names.len() - 1) as u32
            }
        }
    }
}

fn kind_code(kind: NodeKind) -> u32 {
    match kind {
        NodeKind::Literal => 0,
        NodeKind::Regex => 1,
        NodeKind::Sequence => 2,
        NodeKind::Custom => 3,
        NodeKind::InfiniteRepeat => 4,
        NodeKind::FiniteRepeat => 5,
        NodeKind::Expression => 6,
        NodeKind::TakeUntil => 7,
        NodeKind::Block => 8,
        NodeKind::BlockContent => 9,
    }
}
