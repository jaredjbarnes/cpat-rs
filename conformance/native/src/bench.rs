//! The native side of `conformance/bench.mjs`: the same spec and work as
//! `conformance/lib/bench_engine.mjs`, timed with `Instant`. Measurement
//! only: nothing here is asserted.

use std::collections::HashMap;
use std::time::Instant;

use cpat_core::{AutoComplete, AutoCompleteOptions, Grammar, GrammarOptions, Patterns};
use serde_json::{Value, json};

pub fn run(spec: &Value, warmup: usize, runs: usize) -> Value {
    let files = spec.as_array().map_or(&[][..], Vec::as_slice);
    Value::Array(
        files
            .iter()
            .map(|file| bench_file(file, warmup, runs))
            .collect(),
    )
}

fn bench_file(file: &Value, warmup: usize, runs: usize) -> Value {
    let mut built = build(file);
    let parse = cases(&file["parse"]);
    let suggest = cases(&file["suggest"]);
    let mut auto_completes: HashMap<(usize, String), Option<AutoComplete>> = HashMap::new();

    let build_times = time(warmup, runs, || {
        build(file);
    });
    let parse_times = time(warmup, runs, || {
        for (g, rule, input) in &parse {
            if let Some(pattern) = built[*g].as_mut().and_then(|p| p.get_mut(rule)) {
                pattern.exec(input);
            }
        }
    });
    let suggest_times = time(warmup, runs, || {
        for (g, rule, input) in &suggest {
            let auto_complete = auto_completes.entry((*g, rule.clone())).or_insert_with(|| {
                let pattern = built[*g].as_ref()?.get(rule)?.clone();
                AutoComplete::new(pattern, AutoCompleteOptions::default()).ok()
            });
            if let Some(auto_complete) = auto_complete {
                auto_complete.suggest_for(input);
            }
        }
    });
    json!({
        "name": file["name"],
        "build": build_times,
        "parse": parse_times,
        "suggest": suggest_times,
    })
}

fn build(file: &Value) -> Vec<Option<Patterns>> {
    let grammars = file["grammars"].as_array().map_or(&[][..], Vec::as_slice);
    grammars
        .iter()
        .map(|text| {
            Grammar::parse_string(text.as_str().unwrap_or_default(), GrammarOptions::default()).ok()
        })
        .collect()
}

fn cases(list: &Value) -> Vec<(usize, String, String)> {
    let list = list.as_array().map_or(&[][..], Vec::as_slice);
    list.iter()
        .map(|c| {
            let text = |key: &str| c[key].as_str().unwrap_or_default().to_string();
            (
                c["g"].as_u64().unwrap_or(0) as usize,
                text("rule"),
                text("input"),
            )
        })
        .collect()
}

/// `runs` timings in milliseconds, after `warmup` untimed runs.
fn time(warmup: usize, runs: usize, mut work: impl FnMut()) -> Vec<f64> {
    for _ in 0..warmup {
        work();
    }
    (0..runs)
        .map(|_| {
            let start = Instant::now();
            work();
            start.elapsed().as_secs_f64() * 1000.0
        })
        .collect()
}
