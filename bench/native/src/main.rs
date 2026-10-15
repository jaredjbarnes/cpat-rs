//! The native runner of the benchmark (`bench/run.mjs`): the same workloads,
//! signatures and timing policy as `bench/lib/js_runner.mjs`, on cpat-core
//! built in release mode.
//!
//!   cpat-bench verify <out.json> <workload.json>…
//!   cpat-bench time <out.json> <policy> <workload.json>
//!
//! A benchmark, not a test: it measures and reports, and asserts nothing.
//! Peak RSS comes from the orchestrator (`/usr/bin/time -l`), as for the JS
//! engines; peak heap from `cpat-bench-peak`, a separate binary, so its
//! counting allocator never runs while this one times.

use std::process::ExitCode;

use serde_json::{Map, Value, json};

use cpat_bench::timing::Policy;
use cpat_bench::workload::{Kind, Prepared, Workload};
use cpat_bench::write;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let outcome = match args.first().map(String::as_str) {
        Some("verify") if args.len() >= 2 => verify(&args[1], &args[2..]),
        Some("time") if args.len() == 4 => time(&args[1], &args[2], &args[3]),
        _ => Err("usage: cpat-bench verify <out.json> <workload.json>… | \
                  cpat-bench time <out.json> <policy> <workload.json>"
            .to_string()),
    };
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn verify(out: &str, files: &[String]) -> Result<(), String> {
    let mut results = Map::new();
    for file in files {
        let workload = Workload::load(file)?;
        let signature = workload.signature();
        eprintln!("  native verified {}", workload.name);
        results.insert(workload.name.clone(), signature);
    }
    write(out, &Value::Object(results))
}

fn time(out: &str, policy: &str, file: &str) -> Result<(), String> {
    let policy = Policy::parse(policy)?;
    let workload = Workload::load(file)?;
    let samples = match workload.kind {
        Kind::Noop => policy.time(|| {}),
        Kind::Build => policy.time(|| {
            let _ = std::hint::black_box(workload.build());
        }),
        Kind::Parse | Kind::Suggest => {
            let mut prepared: Prepared = workload.prepare()?;
            policy.time(|| prepared.run_once())
        }
    };
    write(
        out,
        &json!({ "name": workload.name, "engine": "native", "samples": { "full": samples } }),
    )
}
