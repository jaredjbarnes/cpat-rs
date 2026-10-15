// The markdown report: summary first, then per-workload detail, the wasm
// cost split, memory, correctness failures and the machine.
import { geomean, summarize } from "./stats.mjs";

const MiB = 1024 * 1024;
const GROUPS = [
  ["build", "Grammar build"],
  ["parse", "Parse: realistic documents"],
  ["tests", "Parse: the harness's own test inputs"],
  ["micro", "Micro"],
  ["suggest", "AutoComplete"],
];

const ms = v => (v == null ? "–" : v < 1 ? v.toFixed(3) : v < 100 ? v.toFixed(2) : v.toFixed(0));
const times = (ts, other) => (ts == null || other == null ? "–" : `${(ts / other).toFixed(2)}×`);
const mib = bytes => (bytes == null ? "–" : (bytes / MiB).toFixed(1));
const rate = (amount, medianMs) => amount / (medianMs / 1000);

function throughput(workload, medianMs) {
  if (workload.kind === "parse" && workload.inputs * workload.reps > 1 && workload.bytes < 64 * 1024) {
    return `${Math.round(rate(workload.inputs * workload.reps, medianMs)).toLocaleString("en-US")} parses/s`;
  }
  if (workload.kind === "parse" || workload.kind === "build") return `${(rate(workload.bytes, medianMs) / MiB).toFixed(2)} MiB/s`;
  if (workload.kind === "suggest") return `${Math.round(rate(workload.inputs * workload.reps, medianMs)).toLocaleString("en-US")} suggestions/s`;
  return "";
}

/** The size column: bytes per iteration, and the input count when it's a
 * batch. */
function size(w) {
  const kb = w.bytes >= MiB ? `${(w.bytes / MiB).toFixed(2)} MiB` : `${(w.bytes / 1024).toFixed(1)} KiB`;
  const count = w.inputs > 1 || w.reps > 1 ? ` (${w.inputs}${w.reps > 1 ? ` ×${w.reps}` : ""})` : "";
  return `${kb}${count}`;
}

export function report({ index, verdicts, timings, machine, policy, wasmSize, date }) {
  const stat = (name, engine, variant = "full") => {
    const samples = timings[engine]?.[name]?.samples?.[variant];
    return samples?.length ? summarize(samples) : null;
  };
  const timed = index.filter(w => w.kind !== "noop" && verdicts[w.name]?.ok && stat(w.name, "ts"));
  const lines = [];
  const push = (...l) => lines.push(...l);

  push(
    "# cpat-rs benchmark: TS v12 vs cpat-rs native vs cpat-wasm",
    "",
    `${date}. ${machine.cpu}, ${machine.memory}, ${machine.os}. Node ${machine.node}, ${machine.rustc}.`,
    `Policy per engine and workload, in its own process: warm up ≥ ${policy.warmupRuns} iterations and ≥ ${policy.warmupMs} ms, then time ≥ ${policy.minRuns} iterations and ≥ ${policy.minMs} ms (at most ${policy.maxRuns}).`,
    "Times are milliseconds per iteration. Speed-up is TS v12's median over the engine's (above 1× the engine is faster).",
    "wasm parses are timed as `exec(text).ast`, the same v12 `Node` tree TS v12's `exec` returns. Every workload runs back to back; builds are also timed releasing what they built (see \"Builds that release what they built\").",
    "",
    "## Summary",
    "",
    "Geometric mean of the speed-ups over the workloads in each group (only workloads all three engines agree on).",
    "",
    "| group | workloads | native vs TS | wasm vs TS | native range | wasm range |",
    "|---|---:|---:|---:|---:|---:|",
  );
  for (const [group, title] of GROUPS) {
    const rows = timed.filter(w => w.group === group);
    if (rows.length === 0) continue;
    const ratios = engine => rows.map(w => stat(w.name, "ts").median / stat(w.name, engine).median);
    const range = r => `${Math.min(...r).toFixed(2)}–${Math.max(...r).toFixed(2)}×`;
    push(`| ${title} | ${rows.length} | ${geomean(ratios("native")).toFixed(2)}× | ${geomean(ratios("wasm")).toFixed(2)}× | ${range(ratios("native"))} | ${range(ratios("wasm"))} |`);
  }
  push("");

  push("| workload | size | TS v12 | native | wasm | native × | wasm × |", "|---|---:|---:|---:|---:|---:|---:|");
  for (const w of timed) {
    const [t, n, a] = ["ts", "native", "wasm"].map(e => stat(w.name, e));
    push(`| ${w.name} | ${size(w)} | ${ms(t.median)} | ${ms(n?.median)} | ${ms(a?.median)} | ${times(t.median, n?.median)} | ${times(t.median, a?.median)} |`);
  }
  const failed = index.filter(w => w.kind !== "noop" && verdicts[w.name] && !verdicts[w.name].ok);
  push("", failed.length === 0 ? "Correctness: all three engines agree on every workload." : `Correctness failures (not timed): ${failed.map(w => w.name).join(", ")}; see below.`, "");

  for (const [group, title] of GROUPS) {
    const rows = timed.filter(w => w.group === group);
    if (rows.length === 0) continue;
    push(`## ${title}`, "", "| workload | what | engine | median | p95 | runs | throughput | × TS |", "|---|---|---|---:|---:|---:|---:|---:|");
    for (const w of rows) {
      const tsMedian = stat(w.name, "ts").median;
      ["ts", "native", "wasm"].forEach((engine, i) => {
        const s = stat(w.name, engine);
        if (s == null) return;
        const first = i === 0 ? `${w.name} | ${w.title}` : " | ";
        const label = { ts: "TS v12", native: "native", wasm: "wasm" }[engine];
        push(`| ${first} | ${label} | ${ms(s.median)} | ${ms(s.p95)} | ${s.runs} | ${throughput(w, s.median)} | ${engine === "ts" ? "" : times(tsMedian, s.median)} |`);
      });
    }
    push("");
  }

  push(
    "## Where wasm's time goes",
    "",
    "Parse workloads, wasm only, from a separate instrumented run of the same work (`split`): `wasm` is the call into wasm plus copying its flat `Uint32Array`s out (`takeExecResult`); `view` is the JS cursor and read-only tree view (`parseResult`); `Node tree` is materializing v12's `Node` objects from the view (`.ast`). Medians of per-iteration sums, so they add up to about the `full` median.",
    "",
    "| workload | full | wasm | view | Node tree | Node tree share | native |",
    "|---|---:|---:|---:|---:|---:|---:|",
  );
  for (const w of timed.filter(x => x.kind === "parse")) {
    const [full, wasm, view, nodes] = ["full", "split.wasm", "split.view", "split.nodes"].map(v => stat(w.name, "wasm", v));
    if ([full, wasm, view, nodes].includes(null)) continue;
    const share = nodes.median / (wasm.median + view.median + nodes.median);
    push(`| ${w.name} | ${ms(full.median)} | ${ms(wasm.median)} | ${ms(view.median)} | ${ms(nodes.median)} | ${(share * 100).toFixed(0)}% | ${ms(stat(w.name, "native")?.median)} |`);
  }
  push("");

  push(
    "## Memory",
    "",
    `Peak RSS of each engine's whole timing process (\`/usr/bin/time -l\`, MiB): the runtime included (\`noop\` is a process that loads the engine and runs nothing), and every iteration, so it is an upper bound. Native's RSS overstates its heap: macOS's malloc keeps freed pages, so it grows with the iteration count while live bytes don't. The **native heap** columns are exact, from a counting allocator (\`cpat-bench-peak\`): the most live bytes while the grammar is built (and AutoComplete made), and the most live bytes above that during one iteration. **wasm linear** is the module's memory after the headline runs (it never shrinks, so it is its high-water mark); for builds that is after hundreds of builds nothing released (next section). wasm binary: ${wasmSize}.`,
    "",
    "| workload | TS v12 RSS | wasm RSS | native RSS | native heap: build | native heap: iteration | wasm linear |",
    "|---|---:|---:|---:|---:|---:|---:|",
  );
  for (const w of [...index.filter(x => x.kind === "noop"), ...timed]) {
    const rss = engine => mib(timings[engine]?.[w.name]?.rssBytes);
    const peak = timings.native?.[w.name]?.peak;
    push(`| ${w.name} | ${rss("ts")} | ${rss("wasm")} | ${rss("native")} | ${mib(peak?.prepareBytes)} | ${mib(peak?.iterationBytes)} | ${mib(timings.wasm?.[w.name]?.wasmMemoryBytes)} |`);
  }
  push("");

  push(
    "## Builds that release what they built",
    "",
    "Build workloads run back to back like every other workload (the headline). wasm-bindgen frees a pattern's wasm memory from a `FinalizationRegistry`, which runs only after a garbage collection and only when the event loop turns, so a synchronous loop of `Grammar.parseString` keeps every grammar it built: wasm memory grows and later builds get slower. `released` times the same builds with a full GC and two event-loop turns between iterations (untimed), for both JS engines; it ran first, in a fresh process. The forced GC leaves caches cold, so small builds look slower there for both engines.",
    "",
    "| workload | TS v12 | TS v12 released | wasm | wasm released | wasm linear: released | wasm linear: back to back |",
    "|---|---:|---:|---:|---:|---:|---:|",
  );
  for (const w of timed.filter(x => x.kind === "build")) {
    const t = timings.wasm?.[w.name];
    push(`| ${w.name} | ${ms(stat(w.name, "ts").median)} | ${ms(stat(w.name, "ts", "released")?.median)} | ${ms(stat(w.name, "wasm")?.median)} | ${ms(stat(w.name, "wasm", "released")?.median)} | ${mib(t?.wasmMemoryReleasedBytes)} | ${mib(t?.wasmMemoryBytes)} |`);
  }
  push("");

  if (failed.length > 0) {
    push("## Correctness failures", "", "Not timed: the engines' results differ (signatures in `verify.json`).", "");
    for (const w of failed) push(`- **${w.name}**: \`${JSON.stringify(verdicts[w.name].differences).slice(0, 600)}\``);
    push("");
  }
  push("## Machine", "", "```", machine.uname, machine.cpu, `${machine.memory}, ${machine.cores}`, `node ${machine.node}`, machine.rustc, machine.wasmPack, "```", "");
  return lines.join("\n");
}
