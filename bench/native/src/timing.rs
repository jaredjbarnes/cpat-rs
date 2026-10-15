//! The timing policy of `bench/lib/timing.mjs`, the same rules: warm up for
//! at least `warmup_runs` iterations and `warmup_ms`, then time at least
//! `min_runs` iterations and `min_ms` in all, stopping at `max_runs`.

use std::time::Instant;

pub struct Policy {
    warmup_runs: usize,
    warmup_ms: f64,
    min_runs: usize,
    min_ms: f64,
    max_runs: usize,
}

impl Policy {
    /// `warmupRuns,warmupMs,minRuns,minMs,maxRuns`, as the orchestrator
    /// passes it to every engine.
    pub fn parse(text: &str) -> Result<Policy, String> {
        let numbers: Vec<f64> = text
            .split(',')
            .map(|n| n.parse::<f64>().map_err(|e| format!("policy {text:?}: {e}")))
            .collect::<Result<_, _>>()?;
        let [warmup_runs, warmup_ms, min_runs, min_ms, max_runs] = numbers[..] else {
            return Err(format!("policy {text:?}: expected 5 numbers"));
        };
        Ok(Policy {
            warmup_runs: warmup_runs as usize,
            warmup_ms,
            min_runs: min_runs as usize,
            min_ms,
            max_runs: max_runs as usize,
        })
    }

    /// Run times in milliseconds.
    pub fn time(&self, mut work: impl FnMut()) -> Vec<f64> {
        let start = Instant::now();
        let mut runs = 0;
        while runs < self.warmup_runs || ms_since(start) < self.warmup_ms {
            work();
            runs += 1;
        }
        let mut samples = Vec::new();
        let mut total = 0.0;
        while samples.len() < self.max_runs
            && (samples.len() < self.min_runs || total < self.min_ms)
        {
            let start = Instant::now();
            work();
            let elapsed = ms_since(start);
            samples.push(elapsed);
            total += elapsed;
        }
        samples
    }
}

fn ms_since(start: Instant) -> f64 {
    start.elapsed().as_secs_f64() * 1000.0
}
