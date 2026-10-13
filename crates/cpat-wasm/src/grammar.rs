//! Building grammars for JS (DESIGN §22). The build is synchronous, as in
//! the core: `js/grammar.mjs` fetches an asynchronous build's imports first
//! (asking `importsOf` what each text imports) and hands a host that
//! answers from memory.

use std::rc::Rc;

use cpat_core::{
    Decorators, Grammar, GrammarFile, GrammarOptions, ImportResolver, Json, Pattern, cpat_grammar,
};
use wasm_bindgen::prelude::*;

use crate::JsPattern;
use crate::host::{GrammarHost, message_of};

/// A built grammar: its rule names in definition order, and their
/// patterns in the same order.
#[wasm_bindgen]
pub struct GrammarRecord {
    names: Vec<String>,
    patterns: Vec<Pattern>,
}

#[wasm_bindgen]
impl GrammarRecord {
    #[wasm_bindgen(getter)]
    pub fn names(&self) -> Vec<String> {
        self.names.clone()
    }

    /// The patterns, in `names` order. Hands them over: call once.
    #[wasm_bindgen(js_name = takePatterns)]
    pub fn take_patterns(&mut self) -> Vec<JsPattern> {
        std::mem::take(&mut self.patterns)
            .into_iter()
            .map(JsPattern::from)
            .collect()
    }
}

/// Builds `text` (v12 `Grammar.parseString`). `origin` is the resource the
/// text came from; `params` are patterns the grammar may use by name;
/// `decorators` names the user's decorators in registration order, which
/// `host.decorate` runs. A `GrammarError` is thrown with its message.
#[wasm_bindgen(js_name = buildGrammar)]
pub fn build_grammar(
    text: &str,
    origin: Option<String>,
    params: Vec<JsPattern>,
    decorators: Vec<String>,
    host: GrammarHost,
) -> Result<GrammarRecord, JsError> {
    let host = Rc::new(host);
    let options = GrammarOptions {
        resolver: Some(Rc::new(HostResolver(Rc::clone(&host)))),
        origin_resource: origin,
        params: params.into_iter().map(|param| param.inner).collect(),
        decorators: host_decorators(&host, decorators),
    };
    let built =
        Grammar::parse_string(text, options).map_err(|error| JsError::new(&error.to_string()))?;
    let (names, patterns) = built.into_iter().unzip();
    Ok(GrammarRecord { names, patterns })
}

/// The resources `text`'s import statements name, in order, or `undefined`
/// when its build would fail before importing (see `Grammar::imports_of`).
#[wasm_bindgen(js_name = importsOf)]
pub fn imports_of(text: &str) -> Option<Vec<String>> {
    Grammar::imports_of(text)
}

/// The cpat grammar itself (v12's exported `grammar`).
#[wasm_bindgen(js_name = cpatGrammar)]
pub fn cpat_grammar_pattern() -> JsPattern {
    cpat_grammar().into()
}

/// Loads imports through the host. The host always answers, so a missing
/// resolver is the host's error, worded as v12 words it.
struct HostResolver(Rc<GrammarHost>);

impl ImportResolver for HostResolver {
    fn resolve(&self, resource: &str, origin: Option<&str>) -> Result<GrammarFile, String> {
        let found = self
            .0
            .resolve(resource, origin.map(str::to_string))
            .map_err(|thrown| message_of(&thrown))?;
        match <[String; 2]>::try_from(found) {
            Ok([resource, expression]) => Ok(GrammarFile {
                resource,
                expression,
            }),
            Err(found) => Err(format!(
                "the import host answered with {} values instead of [resource, expression]",
                found.len()
            )),
        }
    }
}

/// Each named decorator, run by the host. Its argument crosses as JSON.
fn host_decorators(host: &Rc<GrammarHost>, names: Vec<String>) -> Decorators {
    names
        .into_iter()
        .fold(Decorators::new(), |decorators, name| {
            let host = Rc::clone(host);
            let called = name.clone();
            decorators.with(&name, move |pattern: Pattern, arg: Option<&Json>| {
                host.decorate(&called, pattern.into(), arg.map(Json::to_string))
                    .map(|kept| kept.inner)
                    .map_err(|thrown| message_of(&thrown))
            })
        })
}
