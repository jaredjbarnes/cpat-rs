// Summaries of run times (milliseconds).

/** Nearest-rank quantile: an actual sample, never an interpolation. */
export function quantile(samples, q) {
  const sorted = [...samples].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.ceil(q * sorted.length) - 1))];
}

export const summarize = samples => ({
  median: quantile(samples, 0.5),
  p95: quantile(samples, 0.95),
  runs: samples.length,
});

/** Geometric mean, for averaging speed-up ratios. */
export const geomean = values => Math.exp(values.reduce((s, v) => s + Math.log(v), 0) / values.length);
