// Runs workloads in TS v12 (under tsx) or cpat-wasm (through js/index.mjs,
// the published Node API), the same code for both.
//
//   verify: node js_runner.mjs <ts|wasm> verify <out.json> <workload.json>…
//     writes each workload's signature (./signature.mjs)
//   time:   node js_runner.mjs <ts|wasm> time <out.json> <policy> <workload.json>
//     writes run times per variant, peak RSS and (wasm) linear memory
//
// wasm is also timed with its cost split (README "Where wasm's time goes"):
// `full` is `pattern.exec(text).ast` through the public API, the same
// v12 `Node` tree TS v12's `exec` returns, so it is the number compared;
// `split` does the same three steps by hand, timing each:
//   wasm  — the wasm call and copying its flat arrays out (`takeExecResult`)
//   view  — the JS cursor and read-only tree view (`parseResult`)
//   nodes — materializing v12's `Node` tree from the view (`.ast`)
import { readFileSync, writeFileSync } from "node:fs";
import { setFlagsFromString } from "node:v8";
import { runInNewContext } from "node:vm";
import { join } from "node:path";
import { pathToFileURL } from "node:url";
import { v12Api, wasmApi, wasmDir } from "../../conformance/lib/paths.mjs";
import { buildSignature, execSignature, suggestSignature } from "./signature.mjs";
import { parsePolicy, time } from "./timing.mjs";

const [engine, mode, outPath, ...rest] = process.argv.slice(2);

// wasm-bindgen's Node glue keeps its instance private; the benchmark (only)
// catches it on construction to read the linear memory's size.
const instances = [];
const OriginalInstance = WebAssembly.Instance;
WebAssembly.Instance = class extends OriginalInstance {
  constructor(module, imports) {
    super(module, imports);
    instances.push(this);
  }
};

const apiPath = { ts: v12Api, wasm: wasmApi }[engine];
if (apiPath == null || !["verify", "time"].includes(mode)) {
  console.error("usage: js_runner.mjs <ts|wasm> <verify|time> <out.json> …");
  process.exit(2);
}
const { Grammar, AutoComplete } = await import(pathToFileURL(apiPath).href);
const wasmJs = file => import(pathToFileURL(join(wasmDir, "js", file)).href);
const { takeExecResult } = engine === "wasm" ? await wasmJs("taken.mjs") : {};
const { parseResult } = engine === "wasm" ? await wasmJs("pattern.mjs") : {};

// A full collection and a turn of the event loop. wasm-bindgen frees a
// pattern's wasm memory from a FinalizationRegistry, which runs only after
// a collection and only when the event loop turns, so a synchronous loop of
// builds keeps every grammar it built (README, "Builds, back to back and released").
setFlagsFromString("--expose-gc");
const gc = runInNewContext("gc");
const release = async () => {
  gc();
  await new Promise(resolve => setTimeout(resolve, 0));
  await new Promise(resolve => setTimeout(resolve, 0));
};

const load = file => JSON.parse(readFileSync(file, "utf8"));

/** The workload's grammar built, and the work of one iteration. Building
 * and the first AutoComplete happen here, untimed, except in a build
 * workload, where building is the work. */
function prepare(workload) {
  const { kind, grammar, cases, reps } = workload;
  if (kind === "noop") return { work: () => {} };
  if (kind === "build") return { work: () => Grammar.parseString(grammar) };
  const patterns = Grammar.parseString(grammar);
  const each = run => () => {
    for (let r = 0; r < reps; r++) for (const [rule, input] of cases) run(patterns[rule], input, rule);
  };
  if (kind === "suggest") {
    const autoCompletes = {};
    const autoComplete = rule => (autoCompletes[rule] ??= new AutoComplete(patterns[rule]));
    return { patterns, autoComplete, work: each((_, input, rule) => autoComplete(rule).suggestFor(input)) };
  }
  const work = each((pattern, input) => pattern.exec(input).ast);
  return { patterns, work, split: engine === "wasm" ? splitExec(each) : null };
}

/** `exec(text).ast` as wasm's `PatternBase.exec` does it, one step at a
 * time, adding each step's time to `parts`. The timer calls cost ~0.1 µs
 * per input, which only the batches of tiny inputs notice. */
function splitExec(each) {
  const parts = { wasm: 0, view: 0, nodes: 0 };
  const work = each((pattern, input) => {
    const t0 = performance.now();
    const raw = takeExecResult(pattern._path.length === 0 ? pattern._root.exec(input, false) : pattern._root.execAt(pattern._path, input, false));
    const t1 = performance.now();
    const result = parseResult(input, raw, pattern._root, pattern._path);
    const t2 = performance.now();
    void result.ast;
    const t3 = performance.now();
    parts.wasm += t1 - t0;
    parts.view += t2 - t1;
    parts.nodes += t3 - t2;
  });
  return { parts, work };
}

function signature(workload) {
  const { kind, grammar, cases } = workload;
  if (kind === "noop") return {};
  if (kind === "build") return buildSignature(() => Grammar.parseString(grammar));
  const { patterns, autoComplete } = prepare(workload);
  if (kind === "suggest") return { cases: cases.map(([rule, input]) => suggestSignature(() => autoComplete(rule).suggestFor(input))) };
  return { cases: cases.map(([rule, input]) => execSignature(() => patterns[rule].exec(input))) };
}

if (mode === "verify") {
  const results = {};
  for (const file of rest) {
    const workload = load(file);
    results[workload.name] = signature(workload);
    console.error(`  ${engine} verified ${workload.name}`);
  }
  writeFileSync(outPath, JSON.stringify(results));
} else {
  const [policyText, file] = rest;
  const policy = parsePolicy(policyText);
  const workload = load(file);
  const { work, split } = prepare(workload);
  const memory = () => instances[0]?.exports.memory?.buffer.byteLength ?? null;
  const samples = {};
  let wasmMemoryReleasedBytes = null;
  if (workload.kind === "build") {
    // First, builds that release what they built between iterations (as an
    // app that builds a grammar now and then does), while memory is fresh.
    // At most 100 runs: each pays a full GC and two timer turns, untimed.
    samples.released = await time({ ...policy, maxRuns: Math.min(policy.maxRuns, 100) }, work, release);
    wasmMemoryReleasedBytes = memory();
  }
  // The headline: back to back, as every other workload runs.
  samples.full = await time(policy, work);
  const wasmMemoryBytes = memory();
  if (split != null) {
    // Per-iteration step times: reset before each iteration, read after.
    const steps = { wasm: [], view: [], nodes: [] };
    const iteration = () => {
      for (const k in split.parts) split.parts[k] = 0;
      split.work();
      for (const k in steps) steps[k].push(split.parts[k]);
    };
    // The warm-up's iterations record steps too; keep the timed ones.
    const timed = (await time(policy, iteration)).length;
    for (const k in steps) samples[`split.${k}`] = steps[k].slice(-timed);
  }
  writeFileSync(
    outPath,
    JSON.stringify({
      name: workload.name,
      engine,
      samples,
      maxRssBytes: process.resourceUsage().maxRSS * 1024,
      wasmMemoryBytes,
      wasmMemoryReleasedBytes,
    }),
  );
}
