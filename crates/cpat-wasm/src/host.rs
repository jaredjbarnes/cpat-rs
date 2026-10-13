//! The JS objects wasm calls back into, typed. `js/*.mjs` builds each one
//! around the user's functions: it wraps patterns going out in the JS
//! classes, and turns anything thrown into a string before it comes back,
//! so a message never needs `js-sys` to read.

use wasm_bindgen::prelude::*;

use crate::JsPattern;

#[wasm_bindgen]
extern "C" {
    /// `find`'s predicate: `test(pattern)` says whether to stop there.
    pub type PatternPredicate;

    #[wasm_bindgen(method, catch)]
    pub fn test(this: &PatternPredicate, pattern: JsPattern) -> Result<bool, JsValue>;

    /// What a grammar build asks of JS (`js/grammar.mjs`).
    pub type GrammarHost;

    /// The grammar `resource` names, as `[resource, expression]` (v12's
    /// `GrammarFile`), from the grammars fetched ahead of the build or the
    /// user's synchronous resolver. Throws the message to report.
    #[wasm_bindgen(method, catch)]
    pub fn resolve(
        this: &GrammarHost,
        resource: &str,
        origin: Option<String>,
    ) -> Result<Vec<String>, JsValue>;

    /// Runs the user's decorator `name` on `pattern` with the statement's
    /// argument (JSON text, or none) and returns the pattern to keep.
    /// Throws the decorator's message.
    #[wasm_bindgen(method, catch)]
    pub fn decorate(
        this: &GrammarHost,
        name: &str,
        pattern: JsPattern,
        arg: Option<String>,
    ) -> Result<JsPattern, JsValue>;
}

/// What a host threw: the JS side throws strings (see the module docs);
/// anything else is described rather than lost.
pub(crate) fn message_of(thrown: &JsValue) -> String {
    thrown.as_string().unwrap_or_else(|| format!("{thrown:?}"))
}
