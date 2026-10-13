//! The native engine of the conformance harness (`conformance/run.mjs`)
//! and of the benchmark (`conformance/bench.mjs`).
//!
//!   cpat-conformance <cases.json> <out.jsonl> [--perturb]
//!   cpat-conformance --bench <spec.json> <out.json> <warmup> <runs>
//!
//! `--perturb` shifts one reported range on purpose, so the harness can
//! show that it fails on a difference (its self-test).

mod bench;
mod engine;
mod normalize;
mod utf16;

use std::fs;
use std::process::ExitCode;

use engine::Fault;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|a| a == "--bench") {
        return bench_main(&args[1..]);
    }
    let paths: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();
    let [cases, out] = paths[..] else {
        eprintln!("usage: cpat-conformance <cases.json> <out.jsonl> [--perturb]");
        return ExitCode::from(2);
    };
    let fault = if args.iter().any(|a| a == "--perturb") {
        Fault::ShiftFirstRange
    } else {
        Fault::None
    };
    let Some(corpus) = read_json(cases) else {
        return ExitCode::from(2);
    };
    let lines: Vec<String> = engine::run(&corpus, fault)
        .iter()
        .map(|line| line.to_string())
        .collect();
    if let Err(error) = fs::write(out, lines.join("\n") + "\n") {
        eprintln!("can't write {out}: {error}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

fn bench_main(args: &[String]) -> ExitCode {
    let [spec, out, warmup, runs] = args else {
        eprintln!("usage: cpat-conformance --bench <spec.json> <out.json> <warmup> <runs>");
        return ExitCode::from(2);
    };
    let (Ok(warmup), Ok(runs)) = (warmup.parse(), runs.parse()) else {
        eprintln!("warmup and runs must be counts");
        return ExitCode::from(2);
    };
    let Some(spec) = read_json(spec) else {
        return ExitCode::from(2);
    };
    let timings = bench::run(&spec, warmup, runs);
    if let Err(error) = fs::write(out, timings.to_string()) {
        eprintln!("can't write {out}: {error}");
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

fn read_json(path: &str) -> Option<serde_json::Value> {
    let read = fs::read_to_string(path)
        .map_err(|e| e.to_string())
        .and_then(|text| serde_json::from_str(&text).map_err(|e| e.to_string()));
    read.map_err(|error| eprintln!("can't read {path}: {error}"))
        .ok()
}
