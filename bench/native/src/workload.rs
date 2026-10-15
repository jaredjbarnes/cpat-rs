//! One workload file (written by `bench/lib/generate.mjs`), its signature
//! and the work of one timed iteration.

use std::collections::HashMap;
use std::fs;

use cpat_core::{
    AutoComplete, AutoCompleteOptions, Grammar, GrammarError, GrammarOptions, Patterns,
};
use serde_json::{Value, json};

use crate::signature;

#[derive(Clone, Copy)]
pub enum Kind {
    Build,
    Parse,
    Suggest,
    Noop,
}

pub struct Workload {
    pub name: String,
    pub kind: Kind,
    grammar: String,
    /// `(rule, input)`, each run `reps` times per iteration.
    cases: Vec<(String, String)>,
    reps: usize,
}

impl Workload {
    pub fn load(path: &str) -> Result<Workload, String> {
        let text = fs::read_to_string(path).map_err(|e| format!("can't read {path}: {e}"))?;
        let value: Value = serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?;
        let text_of = |v: &Value| v.as_str().unwrap_or_default().to_string();
        let kind = match value["kind"].as_str() {
            Some("build") => Kind::Build,
            Some("parse") => Kind::Parse,
            Some("suggest") => Kind::Suggest,
            Some("noop") => Kind::Noop,
            other => return Err(format!("{path}: unknown kind {other:?}")),
        };
        let cases = value["cases"]
            .as_array()
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .map(|c| (text_of(&c[0]), text_of(&c[1])))
            .collect();
        Ok(Workload {
            name: text_of(&value["name"]),
            kind,
            grammar: text_of(&value["grammar"]),
            cases,
            reps: value["reps"].as_u64().unwrap_or(1) as usize,
        })
    }

    pub fn build(&self) -> Result<Patterns, GrammarError> {
        Grammar::parse_string(&self.grammar, GrammarOptions::default())
    }

    pub fn signature(&self) -> Value {
        match self.kind {
            Kind::Noop => json!({}),
            Kind::Build => signature::build(&self.build()),
            Kind::Parse | Kind::Suggest => match self.prepare() {
                Err(error) => json!({ "threw": error }),
                Ok(mut prepared) => {
                    let cases: Vec<Value> = self
                        .cases
                        .iter()
                        .map(|(rule, input)| prepared.signature(rule, input))
                        .collect();
                    json!({ "cases": cases })
                }
            },
        }
    }

    /// The grammar built and, for suggestions, one AutoComplete per rule:
    /// what happens before timing starts.
    pub fn prepare(&self) -> Result<Prepared<'_>, String> {
        let patterns = self.build().map_err(|e| format!("Error: {e}"))?;
        let mut auto_completes = HashMap::new();
        if let Kind::Suggest = self.kind {
            for (rule, _) in &self.cases {
                if auto_completes.contains_key(rule) {
                    continue;
                }
                let pattern = patterns.get(rule).ok_or(format!("no rule {rule}"))?;
                let auto_complete =
                    AutoComplete::new(pattern.clone(), AutoCompleteOptions::default())
                        .map_err(|e| format!("Error: {e}"))?;
                auto_completes.insert(rule.clone(), auto_complete);
            }
        }
        Ok(Prepared {
            workload: self,
            patterns,
            auto_completes,
        })
    }
}

pub struct Prepared<'w> {
    workload: &'w Workload,
    patterns: Patterns,
    auto_completes: HashMap<String, AutoComplete>,
}

impl Prepared<'_> {
    /// One timed iteration: every case, `reps` times.
    pub fn run_once(&mut self) {
        let workload = self.workload;
        let each = || (0..workload.reps).flat_map(|_| workload.cases.iter());
        if let Kind::Suggest = workload.kind {
            for (rule, input) in each() {
                if let Some(auto_complete) = self.auto_completes.get_mut(rule) {
                    std::hint::black_box(auto_complete.suggest_for(input));
                }
            }
        } else {
            for (rule, input) in each() {
                if let Some(pattern) = self.patterns.get_mut(rule) {
                    std::hint::black_box(pattern.exec(input));
                }
            }
        }
    }

    fn signature(&mut self, rule: &str, input: &str) -> Value {
        if let Kind::Suggest = self.workload.kind {
            return match self.auto_completes.get_mut(rule) {
                Some(auto_complete) => signature::suggest(&auto_complete.suggest_for(input), input),
                None => json!([format!("no rule {rule}")]),
            };
        }
        match self.patterns.get_mut(rule) {
            Some(pattern) => signature::exec(&pattern.exec(input), input),
            None => json!([format!("no rule {rule}")]),
        }
    }
}
