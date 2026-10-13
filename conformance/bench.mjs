// The speed report: grammar build, parse and AutoComplete times for every
// corpus grammar in TS v12, cpat-rs native (release) and cpat-wasm
// (release), with warm-up and repeated runs. It measures and reports; it
// is not a test and asserts nothing.
//
//   node conformance/bench.mjs --out <dir> [--runs 15] [--warmup 3] [--no-build]
//
// Writes <dir>/bench.json (every run time) and <dir>/bench.md (median and
// p90 per grammar, speed-ups over TS v12), and prints the markdown.
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { join } from "node:path";
import { benchSpec } from "./lib/bench_spec.mjs";
import { conformanceDir, nativeBin, repoDir, v12Dir, wasmDir } from "./lib/paths.mjs";

const args = process.argv.slice(2);
const option = (name, fallback) => {
  const at = args.indexOf(name);
  return at < 0 ? fallback : args[at + 1];
};
const out = option("--out", null);
if (out == null) {
  console.error("usage: node conformance/bench.mjs --out <dir> [--runs 15] [--warmup 3] [--no-build]");
  process.exit(2);
}
const runs = Number(option("--runs", 15));
const warmup = Number(option("--warmup", 3));
mkdirSync(out, { recursive: true });

const env = { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}` };
if (!args.includes("--no-build")) {
  execFileSync("cargo", ["build", "--release", "-p", "cpat-conformance"], { cwd: repoDir, stdio: "inherit", env });
  execFileSync("wasm-pack", ["build", "--target", "nodejs", "--release", "--out-dir", "pkg-node"], {
    cwd: wasmDir,
    stdio: "inherit",
    env,
  });
}

const spec = benchSpec();
const specPath = join(out, "bench-spec.json");
writeFileSync(specPath, JSON.stringify(spec));

// One engine at a time, so they don't compete for the machine.
const engineScript = join(conformanceDir, "lib", "bench_engine.mjs");
const timings = {};
for (const engine of ["native", "ts", "wasm"]) {
  const result = join(out, `bench-${engine}.json`);
  console.error(`timing ${engine}…`);
  if (engine === "native") {
    execFileSync(nativeBin, ["--bench", specPath, result, String(warmup), String(runs)], { stdio: "inherit" });
  } else if (engine === "ts") {
    execFileSync("npx", ["--yes", "tsx", engineScript, engine, specPath, result, String(warmup), String(runs)], {
      cwd: v12Dir,
      stdio: "inherit",
    });
  } else {
    execFileSync(process.execPath, [engineScript, engine, specPath, result, String(warmup), String(runs)], {
      stdio: "inherit",
    });
  }
  timings[engine] = JSON.parse(readFileSync(result, "utf8"));
}

const quantile = (times, q) => {
  const sorted = [...times].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.ceil(q * sorted.length) - 1)];
};
const stats = times => ({ median: quantile(times, 0.5), p90: quantile(times, 0.9) });

const rows = spec.map((file, i) => {
  const row = { name: file.name, grammars: file.grammars.length, parseCases: file.parse.length, parseUnits: file.parseUnits, suggestCases: file.suggest.length };
  for (const metric of ["build", "parse", "suggest"]) {
    row[metric] = Object.fromEntries(["ts", "native", "wasm"].map(engine => [engine, stats(timings[engine][i][metric])]));
  }
  return row;
});

const ms = value => (value < 1 ? value.toFixed(3) : value < 100 ? value.toFixed(2) : value.toFixed(0));
const ratio = (ts, other) => `${(ts / other).toFixed(2)}×`;
const cell = s => `${ms(s.median)} (${ms(s.p90)})`;

const lines = [
  "# Speed: TS v12 vs cpat-rs native vs cpat-wasm",
  "",
  `${runs} timed runs after ${warmup} warm-up runs per engine, grammar and metric; times in ms, median (p90).`,
  `Speed-up is TS v12's median over the engine's: above 1× the engine is faster. Node ${process.version},`,
  `${cpus()[0]?.model ?? "unknown CPU"}. A measurement, not a test: nothing here is asserted.`,
  "",
  "- **build**: `Grammar.parseString` of every grammar in the corpus file.",
  "- **parse**: `exec` of every captured input (what the tests ran), on built grammars.",
  "- **suggest**: `AutoComplete.suggestFor` on up to 300 truncations, one AutoComplete per rule.",
  "",
];
for (const metric of ["build", "parse", "suggest"]) {
  lines.push(`## ${metric}`, "");
  const extra = metric === "parse" ? " | inputs | TS M units/s | native M units/s | wasm M units/s" : metric === "suggest" ? " | texts" : " | grammars";
  lines.push(`| grammar | TS v12 | native | wasm | native speed-up | wasm speed-up${extra} |`);
  lines.push(`|---|---:|---:|---:|---:|---:|${extra.split("|").slice(1).map(() => "---:|").join("")}`);
  for (const row of rows) {
    const m = row[metric];
    let tail;
    if (metric === "parse") {
      const rate = s => (row.parseUnits / (s.median / 1000) / 1e6).toFixed(1);
      tail = ` | ${row.parseCases} | ${rate(m.ts)} | ${rate(m.native)} | ${rate(m.wasm)}`;
    } else {
      tail = metric === "suggest" ? ` | ${row.suggestCases}` : ` | ${row.grammars}`;
    }
    lines.push(`| ${row.name} | ${cell(m.ts)} | ${cell(m.native)} | ${cell(m.wasm)} | ${ratio(m.ts.median, m.native.median)} | ${ratio(m.ts.median, m.wasm.median)}${tail} |`);
  }
  lines.push("");
}
const markdown = lines.join("\n");
writeFileSync(join(out, "bench.md"), markdown + "\n");
writeFileSync(join(out, "bench.json"), JSON.stringify({ runs, warmup, node: process.version, rows, timings }, null, 1));
console.log(markdown);
console.log(`written: ${join(out, "bench.md")}, ${join(out, "bench.json")}`);
