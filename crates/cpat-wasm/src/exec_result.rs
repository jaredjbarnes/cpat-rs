use cpat_core::{ParseError, ParseResult, PatternError};
use wasm_bindgen::prelude::*;

use crate::cursor_json::cursor_json;
use crate::flat_tree::FlatTree;
use crate::utf16::Utf16Offsets;

/// A parse result laid out for JS: the tree as flat `Uint32Array`s, so it
/// crosses the wasm boundary as one copy instead of one call per node access.
///
/// `nodes` holds `NODE_STRIDE` numbers per node:
/// `[kind, nameIndex, start, end, parent, childrenAt, childrenLen]`, with
/// positions in UTF-16 offsets and `parent` = `0xFFFFFFFF` for the root.
/// `children` holds node indices; a node's children are
/// `children[childrenAt .. childrenAt + childrenLen]`. Node 0 is the root.
/// Only nodes reachable from the root are included.
///
/// `cursor` is everything v12's cursor reports about the parse, as JSON
/// (see `cursor_json`).
#[wasm_bindgen]
pub struct ExecResult {
    tree: FlatTree,
    error: Option<Vec<u32>>,
    furthest_error: Option<Vec<u32>>,
    error_names: Vec<String>,
    pattern_error: Option<String>,
    missing_name: Option<String>,
    too_deep: Option<Vec<u32>>,
    cursor: String,
}

#[wasm_bindgen]
impl ExecResult {
    #[wasm_bindgen(getter)]
    pub fn matched(&self) -> bool {
        !self.tree.nodes.is_empty()
    }

    #[wasm_bindgen(getter)]
    pub fn nodes(&self) -> Vec<u32> {
        self.tree.nodes.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn children(&self) -> Vec<u32> {
        self.tree.children.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn names(&self) -> Vec<String> {
        self.tree.names.clone()
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

    /// The name a reference looked up and found nowhere, when that is the
    /// `patternError` (JS throws v12's message for it).
    #[wasm_bindgen(getter, js_name = missingName)]
    pub fn missing_name(&self) -> Option<String> {
        self.missing_name.clone()
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

    /// The cursor's report (errors, leaf matches, history) as JSON.
    #[wasm_bindgen(getter)]
    pub fn cursor(&self) -> String {
        self.cursor.clone()
    }
}

impl ExecResult {
    pub fn from_result(text: &str, result: &ParseResult<'_>) -> ExecResult {
        let offsets = Utf16Offsets::new(text);
        let tree = FlatTree::of(result.tree().map(|tree| tree.root()), &offsets);

        let error_of = |error: &ParseError| vec![offsets.of(error.start), offsets.of(error.last)];
        let name_of = |error: &Option<ParseError>| {
            error
                .as_ref()
                .map(|e| e.pattern.name.as_str().to_string())
                .unwrap_or_default()
        };

        ExecResult {
            tree,
            error: result.error.as_ref().map(error_of),
            furthest_error: result.furthest_error.as_ref().map(error_of),
            error_names: vec![name_of(&result.error), name_of(&result.furthest_error)],
            pattern_error: result.pattern_error.as_ref().map(ToString::to_string),
            missing_name: match &result.pattern_error {
                Some(PatternError::UnknownReference { target, .. }) => Some(target.to_string()),
                _ => None,
            },
            // Saturating: a depth past u32 can't be asked for from JS anyway.
            too_deep: result.too_deep.map(|too_deep| {
                vec![
                    u32::try_from(too_deep.max_depth).unwrap_or(u32::MAX),
                    offsets.of(too_deep.pos),
                ]
            }),
            cursor: cursor_json(result, &offsets),
        }
    }
}
