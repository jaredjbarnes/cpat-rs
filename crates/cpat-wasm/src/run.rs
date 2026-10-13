//! Running the pattern at a place: `exec`, `test`, a direct `parse` from a
//! position, and the error message for a parse. A place below the root
//! runs as a root of its own (a clone), as the core runs a pattern taken
//! out of its tree (DESIGN §11).

use cpat_core::{Cursor, Match, PatternPath, generate_error_message_after};
use wasm_bindgen::prelude::*;

use crate::JsPattern;
use crate::exec_result::ExecResult;
use crate::utf16::byte_of;

impl JsPattern {
    /// `run` on the pattern at `path`: the root itself, so what a parse
    /// resolves stays resolved (as on a v12 pattern), or a clone of a
    /// place below it.
    fn run_at<R>(
        &mut self,
        path: &[u32],
        run: impl FnOnce(&mut cpat_core::Pattern) -> R,
    ) -> Option<R> {
        if path.is_empty() {
            return Some(run(&mut self.inner));
        }
        let mut place = self.owned_at(path)?;
        Some(run(&mut place))
    }
}

#[wasm_bindgen(js_class = Pattern)]
impl JsPattern {
    /// v12 `exec(text, record)` on the pattern at `path`.
    #[wasm_bindgen(js_name = execAt)]
    pub fn exec_at(&mut self, path: Vec<u32>, text: &str, record: bool) -> Option<ExecResult> {
        self.run_at(&path, |pattern| {
            let result = if record {
                pattern.exec_recording(text)
            } else {
                pattern.exec(text)
            };
            ExecResult::from_result(text, &result)
        })
    }

    /// v12 `test(text, record)` on the pattern at `path`.
    #[wasm_bindgen(js_name = testAt)]
    pub fn test_at(&mut self, path: Vec<u32>, text: &str, record: bool) -> bool {
        self.run_at(&path, |pattern| {
            if record {
                pattern.test_recording(text)
            } else {
                pattern.test(text)
            }
        })
        .unwrap_or(false)
    }

    /// v12 `parse(cursor)`: the pattern at `path` parses `text` from UTF-16
    /// offset `from`, without `exec`'s implicit scope or its check that the
    /// whole text matched. The tree is whatever the pattern matched.
    #[wasm_bindgen(js_name = parseAt)]
    pub fn parse_at(
        &mut self,
        path: Vec<u32>,
        text: &str,
        from: u32,
        record: bool,
    ) -> Option<ExecResult> {
        self.run_at(&path, |pattern| {
            let mut cursor = Cursor::new(text);
            // v12's `moveTo` lands on the start of the grapheme it names.
            let from = cursor.snap_back(byte_of(text, from));
            cursor.set_pos(from);
            if record {
                cursor.start_recording();
            }
            let root = pattern.parse(&mut cursor);
            let result = cursor.finish(root);
            ExecResult::from_result(text, &result)
        })
    }

    /// v12 `generateErrorMessage(pattern, cursor)` for the pattern at
    /// `path`, after a parse of `text` whose furthest leaf match ends at
    /// UTF-16 offset `leaf_end` with `leaf_path` from that pattern
    /// (`undefined` when nothing matched).
    #[wasm_bindgen(js_name = errorMessageAt)]
    pub fn error_message_at(
        &self,
        path: Vec<u32>,
        text: &str,
        leaf_path: Option<Vec<u32>>,
        leaf_end: u32,
    ) -> Option<String> {
        let pattern = self.owned_at(&path)?;
        let leaf = leaf_path.map(|steps| Match {
            pattern: pattern.meta(),
            start: 0,
            end: byte_of(text, leaf_end),
            path: PatternPath::new(&steps),
        });
        Some(generate_error_message_after(
            &pattern,
            text,
            leaf.as_ref(),
            cpat_core::Limits::default(),
        ))
    }
}
