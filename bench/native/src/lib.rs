//! The native side of the benchmark (`bench/run.mjs`): workloads, their
//! signatures and the timing policy, shared by `cpat-bench` (verify, time)
//! and `cpat-bench-peak` (peak heap).

pub mod signature;
pub mod timing;
pub mod workload;

use std::fs;

use serde_json::Value;

pub fn write(path: &str, value: &Value) -> Result<(), String> {
    fs::write(path, value.to_string()).map_err(|e| format!("can't write {path}: {e}"))
}
