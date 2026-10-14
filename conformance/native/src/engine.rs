//! Runs the expanded corpus (`conformance/lib/corpus.mjs`) through the
//! core and reports each case as `conformance/lib/js_engine.mjs` does.

use std::collections::HashMap;

use cpat_core::{
    AutoComplete, AutoCompleteOptions, Grammar, Pattern, Patterns, generate_error_message,
};
use serde_json::{Map, Value, json};

use crate::normalize::{ast_rows, error_triple};
use crate::replay;
use crate::utf16::Utf16;

/// A deliberate fault for the harness's self-test: the first tree reported
/// gets its root's end moved one unit, which the comparison must flag.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Fault {
    None,
    ShiftFirstRange,
}

pub fn run(corpus: &Value, mut fault: Fault) -> Vec<Value> {
    let mut lines = Vec::new();
    for file in array(&corpus["files"]) {
        for grammar in array(&file["grammars"]) {
            let (mut patterns, build) = build(grammar);
            lines.push(build);
            // One AutoComplete per rule and options, reused as an editor would.
            let mut auto_completes = HashMap::new();
            for case in array(&grammar["cases"]) {
                let mut result = run_case(patterns.as_mut(), case, &mut auto_completes);
                if fault == Fault::ShiftFirstRange && shift_first_range(&mut result) {
                    fault = Fault::None;
                }
                result.insert("id".into(), case["id"].clone());
                lines.push(Value::Object(result));
            }
        }
    }
    lines
}

/// The grammar built as it was captured, and its build line: the error,
/// the rule names, and what its recording decorators received.
fn build(grammar: &Value) -> (Option<Patterns>, Value) {
    let options = &grammar["options"];
    let built = replay::options(options).map(|(grammar_options, decorated)| {
        let built = Grammar::parse_string(str_of(&grammar["text"]), grammar_options)
            // v12 throws an `Error` whose message is the grammar error.
            .map_err(|error| format!("Error: {error}"));
        (built, decorated)
    });
    let (patterns, error, decorated) = match built {
        Ok((Ok(patterns), decorated)) => (Some(patterns), Value::Null, decorated),
        Ok((Err(error), decorated)) => (None, Value::from(error), decorated),
        Err(error) => (None, Value::from(error), Default::default()),
    };
    let mut line = object(json!({
        "build": grammar["key"],
        "error": error,
        "rules": patterns.as_ref().map(|p| p.names().collect::<Vec<_>>()),
    }));
    if !options["decorators"].is_null() {
        line.insert("decorated".into(), Value::from(decorated.take()));
    }
    (patterns, Value::Object(line))
}

type AutoCompletes = HashMap<String, Result<AutoComplete, String>>;

fn run_case(
    patterns: Option<&mut Patterns>,
    case: &Value,
    auto_completes: &mut AutoCompletes,
) -> Map<String, Value> {
    let rule = str_of(&case["rule"]);
    let Some(pattern) = patterns.and_then(|patterns| patterns.get_mut(rule)) else {
        return object(json!({ "missing": true }));
    };
    let input = str_of(&case["input"]);
    let mut result = object(json!({ "exec": exec_case(pattern, input) }));
    let options = &case["autocomplete"];
    if !options.is_null() {
        let key = format!("{rule}\u{0}{options}");
        let auto_complete = auto_completes
            .entry(key)
            .or_insert_with(|| auto_complete_for(pattern, options));
        result.insert(
            "autocomplete".into(),
            auto_complete_case(auto_complete, input),
        );
    }
    result
}

fn exec_case(pattern: &mut Pattern, input: &str) -> Value {
    let result = pattern.exec(input);
    if let Some(error) = &result.pattern_error {
        return json!({ "threw": format!("Error: {error}") });
    }
    let offsets = Utf16::new(input);
    let mut out = object(json!({
        "matched": result.tree.is_some(),
        "ast": ast_rows(result.tree.as_ref(), input, &offsets),
        "error": error_triple(result.error.as_ref(), &offsets),
        "furthest": error_triple(result.furthest_error.as_ref(), &offsets),
        "message": generate_error_message(pattern, &result),
    }));
    if result.too_deep.is_some() {
        out.insert("tooDeep".into(), Value::Bool(true));
    }
    Value::Object(out)
}

fn auto_complete_for(pattern: &Pattern, options: &Value) -> Result<AutoComplete, String> {
    let options = AutoCompleteOptions {
        greedy_pattern_names: array(&options["greedyPatternNames"])
            .iter()
            .map(|name| str_of(name).to_string())
            .collect(),
        custom_tokens: options["customTokens"]
            .as_object()
            .map(|tokens| {
                tokens
                    .iter()
                    .map(|(name, list)| {
                        let list = array(list).iter().map(|t| str_of(t).to_string());
                        (name.clone(), list.collect())
                    })
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default(),
        disable_dedupe: options["disableDedupe"].as_bool().unwrap_or(false),
    };
    AutoComplete::new(pattern.clone(), options).map_err(|error| format!("Error: {error}"))
}

fn auto_complete_case(auto_complete: &mut Result<AutoComplete, String>, input: &str) -> Value {
    let auto_complete = match auto_complete {
        Ok(auto_complete) => auto_complete,
        Err(error) => return json!({ "threw": error }),
    };
    let suggestion = auto_complete.suggest_for(input);
    let offsets = Utf16::new(input);
    let mut out = object(json!({
        "complete": suggestion.is_complete,
        "at": suggestion.error_at_index.map(|at| offsets.of(at)),
        "options": suggestion
            .options
            .iter()
            .map(|option| json!([option.text, offsets.of(option.start_index)]))
            .collect::<Vec<_>>(),
    }));
    if suggestion.too_deep().is_some() {
        out.insert("tooDeep".into(), Value::Bool(true));
    }
    Value::Object(out)
}

/// Moves the reported root's end by one; false when the case has no tree.
fn shift_first_range(result: &mut Map<String, Value>) -> bool {
    let Some(Value::String(rows)) = result.get_mut("exec").and_then(|exec| exec.get_mut("ast"))
    else {
        return false;
    };
    let first = rows.split('\n').next().unwrap_or_default();
    let mut fields: Vec<String> = first.split('|').map(str::to_string).collect();
    let end: u64 = fields[4].parse().unwrap_or(0);
    fields[4] = (end + 1).to_string();
    *rows = format!("{}{}", fields.join("|"), &rows[first.len()..]);
    true
}

fn array(value: &Value) -> &[Value] {
    value.as_array().map_or(&[], Vec::as_slice)
}

fn str_of(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fault_moves_the_first_tree_roots_end_by_one() {
        let mut result = object(json!({ "exec": { "ast": "0|sequence|s|0|3\n1|literal|a|0|1" } }));
        assert!(shift_first_range(&mut result));
        assert_eq!(result["exec"]["ast"], "0|sequence|s|0|4\n1|literal|a|0|1");
    }

    #[test]
    fn the_fault_waits_for_a_case_with_a_tree() {
        let mut result = object(json!({ "exec": { "ast": null } }));
        assert!(!shift_first_range(&mut result));
    }

    #[test]
    fn a_corpus_runs_through_the_core() {
        let corpus = json!({ "files": [{ "grammars": [{
            "key": "g#0",
            "text": "word = /[a-z]+/\nwords = (word, \" \")+",
            "cases": [
                { "id": 0, "rule": "words", "input": "ab cd", "autocomplete": null },
                { "id": 1, "rule": "words", "input": "ab 1", "autocomplete": {} },
            ],
        }] }] });
        let lines = run(&corpus, Fault::None);
        assert_eq!(lines[0]["rules"], json!(["word", "words"]));
        assert_eq!(lines[1]["exec"]["matched"], true);
        assert_eq!(
            lines[1]["exec"]["ast"].as_str().unwrap().lines().next(),
            Some("0|infinite-repeat|words|0|5")
        );
        assert_eq!(lines[2]["exec"]["matched"], false);
        assert_eq!(lines[2]["autocomplete"]["complete"], false);
    }

    /// The keys `conformance/lib/normalize.mjs` writes (and
    /// `conformance/test/harness.test.mjs` pins), so a field dropped here is
    /// a test failure, not a silent hole in the comparison.
    #[test]
    fn every_compared_field_is_written() {
        let deep = "(".repeat(1100);
        let corpus = json!({ "files": [{ "grammars": [{
            "key": "g#0",
            "text": "@mark\nword = /[a-z]+/\nnest = \"(\" + nest + \")\" | word",
            "options": { "entry": "parseString", "decorators": ["mark"] },
            "cases": [
                { "id": 0, "rule": "word", "input": "ab1", "autocomplete": {} },
                { "id": 1, "rule": "nest", "input": deep, "autocomplete": {} },
            ],
        }] }] });
        let lines = run(&corpus, Fault::None);
        fn keys(value: &Value) -> Vec<&str> {
            let mut keys: Vec<&str> = value
                .as_object()
                .unwrap()
                .keys()
                .map(String::as_str)
                .collect();
            keys.sort();
            keys
        }
        assert_eq!(keys(&lines[0]), ["build", "decorated", "error", "rules"]);
        assert_eq!(
            lines[0]["decorated"],
            json!([["mark", "word", "regex", null]])
        );
        assert_eq!(
            keys(&lines[1]["exec"]),
            ["ast", "error", "furthest", "matched", "message"]
        );
        assert_eq!(
            keys(&lines[1]["autocomplete"]),
            ["at", "complete", "options"]
        );
        assert_eq!(
            keys(&lines[2]["exec"]),
            ["ast", "error", "furthest", "matched", "message", "tooDeep"]
        );
        assert_eq!(
            keys(&lines[2]["autocomplete"]),
            ["at", "complete", "options", "tooDeep"]
        );
    }
}
