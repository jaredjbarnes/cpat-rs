// The one timing policy every runner follows (bench/native/src/timing.rs
// mirrors it): warm up for at least `warmupRuns` iterations and
// `warmupMs`, then time at least `minRuns` iterations and `minMs` in all,
// stopping at `maxRuns`. The orchestrator passes it as one argument,
// `warmupRuns,warmupMs,minRuns,minMs,maxRuns`, so the engines can't drift.

export function parsePolicy(text) {
  const [warmupRuns, warmupMs, minRuns, minMs, maxRuns] = text.split(",").map(Number);
  return { warmupRuns, warmupMs, minRuns, minMs, maxRuns };
}

/** Run times in milliseconds. `between`, when given, runs after every
 * iteration, warm-up included, untimed and awaited. */
export async function time(policy, work, between = null) {
  let start = performance.now();
  for (let i = 0; i < policy.warmupRuns || performance.now() - start < policy.warmupMs; i++) {
    work();
    if (between) await between();
  }
  const samples = [];
  let total = 0;
  while (samples.length < policy.maxRuns && (samples.length < policy.minRuns || total < policy.minMs)) {
    start = performance.now();
    work();
    const elapsed = performance.now() - start;
    samples.push(elapsed);
    total += elapsed;
    if (between) await between();
  }
  return samples;
}
