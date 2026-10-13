//! v12's `AutoComplete` for JS, with UTF-16 offsets.

use std::collections::HashMap;

use cpat_core::{AutoComplete, AutoCompleteOptions, SuggestionOption};
use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use crate::JsPattern;
use crate::flat_tree::FlatTree;
use crate::utf16::Utf16Offsets;

/// Suggests what could come next in a text, for the pattern it was built
/// with (a copy: later changes to that pattern don't reach it).
#[wasm_bindgen(js_name = AutoComplete)]
pub struct JsAutoComplete {
    inner: AutoComplete,
}

#[wasm_bindgen(js_class = AutoComplete)]
impl JsAutoComplete {
    /// `custom_tokens` is a JSON object of token lists by pattern name.
    /// Throws when the pattern names a reference that resolves to nothing.
    #[wasm_bindgen(constructor)]
    pub fn new(
        pattern: &JsPattern,
        greedy_pattern_names: Vec<String>,
        custom_tokens: &str,
        disable_dedupe: bool,
    ) -> Result<JsAutoComplete, JsError> {
        let custom_tokens: HashMap<String, Vec<String>> = serde_json::from_str(custom_tokens)
            .map_err(|error| JsError::new(&format!("customTokens: {error}")))?;
        let options = AutoCompleteOptions {
            greedy_pattern_names,
            custom_tokens,
            disable_dedupe,
        };
        let inner = AutoComplete::new(pattern.inner.clone(), options)
            .map_err(|error| JsError::new(&error.to_string()))?;
        Ok(JsAutoComplete { inner })
    }

    /// The suggestion for `text`, as JSON (see `suggestion_json`); its
    /// tree, when the parse matched anything, in `tree`'s flat arrays.
    #[wasm_bindgen(js_name = suggestFor)]
    pub fn suggest_for(&mut self, text: &str) -> SuggestionResult {
        let suggestion = self.inner.suggest_for(text);
        let offsets = Utf16Offsets::new(text);
        let error = suggestion.error.as_ref().map(|error| {
            json!({
                "start": offsets.of(error.start),
                "last": offsets.of(error.last),
                "name": error.pattern.name.as_str(),
                "id": error.pattern.id.to_string(),
            })
        });
        let options: Vec<Value> = suggestion
            .options
            .iter()
            .map(|option| option_json(option, &offsets))
            .collect();
        let json = json!({
            "isComplete": suggestion.is_complete,
            "options": options,
            "error": error,
            "errorAtIndex": suggestion.error_at_index.map(|at| offsets.of(at)),
            "patternError": suggestion.pattern_error.as_ref().map(ToString::to_string),
        });
        SuggestionResult {
            json: json.to_string(),
            tree: FlatTree::of(suggestion.tree.as_ref().map(|tree| tree.root()), &offsets),
        }
    }
}

fn option_json(option: &SuggestionOption, offsets: &Utf16Offsets) -> Value {
    let segments: Vec<Value> = option
        .suggestion_sequence
        .iter()
        .map(|segment| {
            json!({
                "text": segment.text,
                "name": segment.pattern.name,
                "path": segment.pattern.path.steps(),
            })
        })
        .collect();
    json!({
        "text": option.text,
        "startIndex": offsets.of(option.start_index),
        "suggestionSequence": segments,
    })
}

/// A suggestion laid out for JS.
#[wasm_bindgen]
pub struct SuggestionResult {
    json: String,
    tree: FlatTree,
}

#[wasm_bindgen]
impl SuggestionResult {
    /// `{ isComplete, options, error, errorAtIndex, patternError }`.
    #[wasm_bindgen(getter)]
    pub fn json(&self) -> String {
        self.json.clone()
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
}
