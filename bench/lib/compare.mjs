// Compares the three engines' signatures for each workload. A workload is
// timed only when all three agree; otherwise it is a correctness failure,
// reported with the first cases that differ.

const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);

/** `{ ok: true }` or `{ ok: false, differences: [...] }` against TS v12. */
export function compareWorkload(name, signatures) {
  const ts = signatures.ts[name];
  const differences = [];
  for (const engine of ["native", "wasm"]) {
    const other = signatures[engine][name];
    if (ts == null || other == null) {
      differences.push({ engine, missing: ts == null ? "ts" : engine });
      continue;
    }
    if (ts.cases == null || other.cases == null) {
      if (!same(ts, other)) differences.push({ engine, ts, got: other });
      continue;
    }
    if (ts.cases.length !== other.cases.length) {
      differences.push({ engine, ts: `${ts.cases.length} cases`, got: `${other.cases.length} cases` });
      continue;
    }
    const at = ts.cases.flatMap((c, i) => (same(c, other.cases[i]) ? [] : [i]));
    if (at.length > 0) {
      differences.push({ engine, cases: at.length, first: at.slice(0, 3).map(i => ({ case: i, ts: ts.cases[i], got: other.cases[i] })) });
    }
  }
  return differences.length === 0 ? { ok: true } : { ok: false, differences };
}
