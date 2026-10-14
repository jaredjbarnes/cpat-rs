//! Rebuilds a captured grammar's options from their record, as
//! `conformance/lib/replay.mjs` does: params by value, imports answered
//! from the recorded resolver calls, and each custom decorator replaced by
//! one that records what it received.
//!
//! The core's builds are synchronous, so a grammar v12 built with the async
//! `Grammar.parse` is rebuilt here with its async resolver's answers given
//! synchronously, which is what the core's hosts do (DESIGN §8).

use std::cell::RefCell;
use std::rc::Rc;

use cpat_core::{Decorators, GrammarFile, GrammarOptions, Literal, Pattern, PatternError, Regex};
use serde_json::{Value, json};

use crate::js_json::stringify;

/// What the recording decorators received, in the order they ran.
pub type Decorated = Rc<RefCell<Vec<Value>>>;

/// The options `record` describes, and the list its decorators fill. A
/// param the core refuses to construct is the build's error.
pub fn options(record: &Value) -> Result<(GrammarOptions, Decorated), String> {
    let decorated = Decorated::default();
    let mut options = GrammarOptions {
        origin_resource: record["originResource"].as_str().map(str::to_string),
        ..GrammarOptions::default()
    };
    for param in array(&record["params"]) {
        options
            .params
            .push(param_of(param).map_err(|error| format!("Error: {error}"))?);
    }
    let mut decorators = Decorators::new();
    for name in array(&record["decorators"])
        .iter()
        .filter_map(Value::as_str)
    {
        let log = decorated.clone();
        let decorator = name.to_string();
        decorators = decorators.with(name, move |pattern: Pattern, arg: Option<&Value>| {
            log.borrow_mut().push(json!([
                decorator,
                pattern.name(),
                pattern.type_name(),
                arg.map(stringify),
            ]));
            Ok(pattern)
        });
    }
    options.decorators = decorators;
    // v12 calls the async resolver in `parse` and the sync one in
    // `parseString`; a record has the one its build used.
    let calls = match record["entry"].as_str() {
        Some("parse") => &record["resolveImport"],
        _ => &record["resolveImportSync"],
    };
    if let Some(calls) = calls.as_array() {
        let calls = calls.clone();
        options = options.with_resolver(move |resource: &str, origin: Option<&str>| {
            answer(&calls, resource, origin)
        });
    }
    Ok((options, decorated))
}

fn param_of(param: &Value) -> Result<Pattern, PatternError> {
    let name = str_of(&param["name"]);
    match str_of(&param["type"]) {
        "literal" => Literal::new(name, str_of(&param["value"])).map(Pattern::from),
        _ => Regex::new(name, str_of(&param["regex"])).map(Pattern::from),
    }
}

/// The recorded answer to `resource` asked from `origin`, with replay.mjs's
/// words for an import nothing recorded.
fn answer(calls: &[Value], resource: &str, origin: Option<&str>) -> Result<GrammarFile, String> {
    let call = calls
        .iter()
        .find(|call| call["resource"] == resource && call["origin"].as_str() == origin);
    let Some(call) = call else {
        return Err(format!(
            "unrecorded import {} from {}",
            Value::from(resource),
            origin.map_or(Value::Null, Value::from)
        ));
    };
    if let Some(error) = call["error"].as_str() {
        return Err(error.to_string());
    }
    Ok(GrammarFile {
        resource: str_of(&call["file"]["resource"]).to_string(),
        expression: str_of(&call["file"]["expression"]).to_string(),
    })
}

fn array(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

fn str_of(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpat_core::Grammar;

    #[test]
    fn imports_are_answered_from_the_record_and_others_are_errors() {
        let record = json!({
            "entry": "parse",
            "resolveImport": [
                { "resource": "a", "origin": null, "file": { "resource": "a", "expression": "x = \"X\"" } },
                { "resource": "b", "origin": null, "error": "No Import" },
            ],
        });
        let (options, _) = options(&record).unwrap();
        let built = Grammar::parse_string("import { x } from \"a\"\ny = x", options.clone());
        assert!(built.unwrap().get_mut("y").unwrap().test("X"));
        let refused = Grammar::parse_string("import { x } from \"b\"\ny = x", options.clone());
        assert_eq!(refused.unwrap_err().to_string(), "No Import");
        let unknown = Grammar::parse_string("import { x } from \"c\"\ny = x", options);
        assert_eq!(
            unknown.unwrap_err().to_string(),
            "unrecorded import \"c\" from null"
        );
    }

    #[test]
    fn decorators_record_what_they_received_and_params_are_built_by_value() {
        let record = json!({
            "entry": "parseString",
            "decorators": ["config"],
            "params": [{ "type": "literal", "name": "value", "value": "v" }],
        });
        let (options, decorated) = options(&record).unwrap();
        let mut built = Grammar::parse_string(
            "use params { value }\n@config({\"b\": 1.5, \"a\": [2.0]})\n@config\nx = value + \"!\"",
            options,
        )
        .unwrap();
        assert!(built.get_mut("x").unwrap().test("v!"));
        assert_eq!(
            *decorated.borrow(),
            [
                json!(["config", "x", "sequence", null]),
                json!(["config", "x", "sequence", "{\"b\":1.5,\"a\":[2]}"]),
            ]
        );
    }
}
