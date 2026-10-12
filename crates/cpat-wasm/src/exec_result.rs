use cpat_core::{Node, NodeKind, ParseError, ParseResult};
use wasm_bindgen::prelude::*;

use crate::utf16::Utf16Offsets;

/// Fields per node in `nodes`.
pub const NODE_STRIDE: usize = 7;
const NONE: u32 = u32::MAX;

/// A parse result laid out for JS: the tree as flat `Uint32Array`s, so it
/// crosses the wasm boundary as one copy instead of one call per node access.
///
/// `nodes` holds `NODE_STRIDE` numbers per node:
/// `[kind, nameIndex, start, end, parent, childrenAt, childrenLen]`, with
/// positions in UTF-16 offsets and `parent` = `0xFFFFFFFF` for the root.
/// `children` holds node indices; a node's children are
/// `children[childrenAt .. childrenAt + childrenLen]`. Node 0 is the root.
/// Only nodes reachable from the root are included.
#[wasm_bindgen]
pub struct ExecResult {
    nodes: Vec<u32>,
    children: Vec<u32>,
    names: Vec<String>,
    error: Option<Vec<u32>>,
    furthest_error: Option<Vec<u32>>,
    error_names: Vec<String>,
    pattern_error: Option<String>,
    too_deep: Option<Vec<u32>>,
}

#[wasm_bindgen]
impl ExecResult {
    #[wasm_bindgen(getter)]
    pub fn matched(&self) -> bool {
        !self.nodes.is_empty()
    }

    #[wasm_bindgen(getter)]
    pub fn nodes(&self) -> Vec<u32> {
        self.nodes.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn children(&self) -> Vec<u32> {
        self.children.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn names(&self) -> Vec<String> {
        self.names.clone()
    }

    /// `[start, lastIndex]` in UTF-16 offsets, or `undefined`.
    #[wasm_bindgen(getter)]
    pub fn error(&self) -> Option<Vec<u32>> {
        self.error.clone()
    }

    #[wasm_bindgen(getter, js_name = errorPattern)]
    pub fn error_pattern(&self) -> Option<String> {
        self.error.as_ref().map(|_| self.error_names[0].clone())
    }

    /// `[start, lastIndex]` of the error that got furthest, or `undefined`.
    #[wasm_bindgen(getter, js_name = furthestError)]
    pub fn furthest_error(&self) -> Option<Vec<u32>> {
        self.furthest_error.clone()
    }

    /// Why the pattern couldn't run (a reference that names nothing), or
    /// `undefined`. When set, nothing was parsed.
    #[wasm_bindgen(getter, js_name = patternError)]
    pub fn pattern_error(&self) -> Option<String> {
        self.pattern_error.clone()
    }

    /// `[maxDepth, position]` (UTF-16) when the input nested deeper than
    /// the parser allows and the parse stopped there, or `undefined`. Deep
    /// input is an answer, not a stack overflow: the module keeps working.
    #[wasm_bindgen(getter, js_name = tooDeep)]
    pub fn too_deep(&self) -> Option<Vec<u32>> {
        self.too_deep.clone()
    }

    #[wasm_bindgen(getter, js_name = furthestErrorPattern)]
    pub fn furthest_error_pattern(&self) -> Option<String> {
        self.furthest_error
            .as_ref()
            .map(|_| self.error_names[1].clone())
    }
}

impl ExecResult {
    pub fn from_result(text: &str, result: &ParseResult<'_>) -> ExecResult {
        let offsets = Utf16Offsets::new(text);
        let mut flat = Flattener {
            offsets: &offsets,
            nodes: Vec::new(),
            children: Vec::new(),
            names: Vec::new(),
        };

        if let Some(tree) = result.tree() {
            flat.add(tree.root(), NONE);
        }

        let error_of = |error: &ParseError| vec![offsets.of(error.start), offsets.of(error.last)];
        let name_of = |error: Option<ParseError>| {
            error
                .map(|e| e.pattern.name.as_str().to_string())
                .unwrap_or_default()
        };

        ExecResult {
            nodes: flat.nodes,
            children: flat.children,
            names: flat.names,
            error: result.error.as_ref().map(error_of),
            furthest_error: result.furthest_error.as_ref().map(error_of),
            error_names: vec![name_of(result.error), name_of(result.furthest_error)],
            pattern_error: result.pattern_error.as_ref().map(ToString::to_string),
            // Saturating: a depth past u32 can't be asked for from JS anyway.
            too_deep: result.too_deep.map(|too_deep| {
                vec![
                    u32::try_from(too_deep.max_depth).unwrap_or(u32::MAX),
                    offsets.of(too_deep.pos),
                ]
            }),
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
    /// Writes `node` and its subtree; returns the node's index. A node's
    /// children list is reserved before its children are written, so each
    /// list stays contiguous.
    fn add(&mut self, node: Node<'_, '_>, parent: u32) -> u32 {
        let index = (self.nodes.len() / NODE_STRIDE) as u32;
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

        for (i, child) in node.children().enumerate() {
            let child_index = self.add(child, index);
            self.children[children_at + i] = child_index;
        }

        index
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
    }
}
