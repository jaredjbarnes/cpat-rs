// The benchmark: TS v12 vs cpat-rs native (release) vs cpat-wasm (release,
// from Node). It measures and reports; it is not a test and asserts
// nothing. See bench/README.md.
//
//   node bench/run.mjs [--out target/bench] [--only <regex>] [--no-build]
//                      [--policy warmupRuns,warmupMs,minRuns,minMs,maxRuns] [--quick]
//
// 1. builds the native runner and pkg-node in release mode
// 2. generates the workloads (bench/lib/generate.mjs, under TS v12)
// 3. verifies: every engine computes every workload's signature; a
//    workload whose signatures differ is a correctness failure, not timed
// 4. times each agreeing workload, one process per engine and workload,
//    under /usr/bin/time -l for peak RSS
// 5. writes <out>/bench.md, <out>/results.json and prints the markdown
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { cpus } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";
import { repoDir, v12Dir, wasmDir } from "../conformance/lib/paths.mjs";
import { compareWorkload } from "./lib/compare.mjs";
import { report } from "./lib/report.mjs";
import { parsePolicy } from "./lib/timing.mjs";

const benchDir = dirname(fileURLToPath(import.meta.url));
const args = process.argv.slice(2);
const option = (name, fallback) => {
  const at = args.indexOf(name);
  return at < 0 ? fallback : args[at + 1];
};
const out = join(repoDir, option("--out", join("target", "bench")));
const only = new RegExp(option("--only", "."));
const policyText = args.includes("--quick") ? "1,0,3,0,3" : option("--policy", "3,500,10,2000,1000");
const policy = parsePolicy(policyText);
mkdirSync(out, { recursive: true });

const env = { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}` };
const nativeBin = join(repoDir, "target", "release", "cpat-bench");
const nativePeakBin = join(repoDir, "target", "release", "cpat-bench-peak");
const jsRunner = join(benchDir, "lib", "js_runner.mjs");
const run = (command, argv, options = {}) => execFileSync(command, argv, { stdio: "inherit", env, ...options });

if (!args.includes("--no-build")) {
  run("cargo", ["build", "--release", "-p", "cpat-bench"], { cwd: repoDir });
  run("wasm-pack", ["build", "--target", "nodejs", "--release", "--out-dir", "pkg-node"], { cwd: wasmDir });
}

console.error("generating workloads…");
run("npx", ["--yes", "tsx", join(benchDir, "lib", "generate.mjs"), out], { cwd: v12Dir });
const index = JSON.parse(readFileSync(join(out, "index.json"), "utf8")).filter(w => w.kind === "noop" || only.test(w.name));
const workloadFile = w => join(out, "workloads", w.file);

/** The command that runs `engine` with runner arguments `argv`. */
function command(engine, argv) {
  if (engine === "native") return { file: nativeBin, argv, cwd: repoDir };
  if (engine === "ts") return { file: "npx", argv: ["--yes", "tsx", jsRunner, "ts", ...argv], cwd: v12Dir };
  return { file: process.execPath, argv: [jsRunner, "wasm", ...argv], cwd: repoDir };
}

console.error("verifying…");
const signatures = {};
for (const engine of ["ts", "native", "wasm"]) {
  const path = join(out, `verify-${engine}.json`);
  const c = command(engine, ["verify", path, ...index.map(workloadFile)]);
  run(c.file, c.argv, { cwd: c.cwd });
  signatures[engine] = JSON.parse(readFileSync(path, "utf8"));
}
const verdicts = Object.fromEntries(index.map(w => [w.name, compareWorkload(w.name, signatures)]));
writeFileSync(join(out, "verify.json"), JSON.stringify({ verdicts, signatures }, null, 1));
for (const [name, verdict] of Object.entries(verdicts)) if (!verdict.ok) console.error(`  DIFFERS, not timed: ${name}`);

console.error("timing…");
const timings = { ts: {}, native: {}, wasm: {} };
for (const w of index.filter(w => verdicts[w.name].ok)) {
  // Engines take turns per workload, so slow drift in the machine's state
  // (heat, background work) lands on all three alike.
  for (const engine of ["native", "ts", "wasm"]) {
    const path = join(out, "timings", `${w.file.replace(/\.json$/, "")}.${engine}.json`);
    mkdirSync(dirname(path), { recursive: true });
    const c = command(engine, ["time", path, policyText, workloadFile(w)]);
    const result = spawnSync("/usr/bin/time", ["-l", c.file, ...c.argv], { cwd: c.cwd, env, encoding: "utf8", maxBuffer: 1 << 26 });
    if (result.status !== 0) {
      console.error(`  ${engine} failed on ${w.name}:\n${result.stderr.slice(-2000)}`);
      continue;
    }
    const rss = /(\d+)\s+maximum resident set size/.exec(result.stderr);
    timings[engine][w.name] = { ...JSON.parse(readFileSync(path, "utf8")), rssBytes: rss ? Number(rss[1]) : null };
    const median = [...timings[engine][w.name].samples.full].sort((a, b) => a - b)[Math.floor(timings[engine][w.name].samples.full.length / 2)];
    console.error(`  ${w.name.padEnd(30)} ${engine.padEnd(6)} ${median.toFixed(3)} ms`);
  }
  // Native peak heap, from a counting allocator in its own binary (untimed).
  if (timings.native[w.name] != null) {
    const path = join(out, "timings", `${w.file.replace(/\.json$/, "")}.native-peak.json`);
    run(nativePeakBin, [path, workloadFile(w)], { cwd: repoDir });
    timings.native[w.name].peak = JSON.parse(readFileSync(path, "utf8"));
  }
}

const shell = (file, argv) => {
  try {
    return execFileSync(file, argv, { encoding: "utf8", env }).trim();
  } catch {
    return "unknown";
  }
};
const machine = {
  uname: shell("uname", ["-a"]),
  cpu: shell("sysctl", ["-n", "machdep.cpu.brand_string"]),
  memory: `${Math.round(Number(shell("sysctl", ["-n", "hw.memsize"])) / 2 ** 30)} GiB RAM`,
  cores: `${cpus().length} cores (${shell("sysctl", ["-n", "hw.perflevel0.physicalcpu"])} performance, ${shell("sysctl", ["-n", "hw.perflevel1.physicalcpu"])} efficiency)`,
  os: `macOS ${shell("sw_vers", ["-productVersion"])}`,
  node: process.version,
  rustc: shell("rustc", ["-V"]),
  wasmPack: shell("wasm-pack", ["--version"]),
};
const wasmFile = join(wasmDir, "pkg-node", "cpat_wasm_bg.wasm");
const wasmBytes = readFileSync(wasmFile);
const wasmSize = `${(statSync(wasmFile).size / 1024).toFixed(0)} KiB (${(gzipSync(wasmBytes, { level: 9 }).length / 1024).toFixed(0)} KiB gzipped), pkg-node release`;

const markdown = report({ index, verdicts, timings, machine, policy, wasmSize, date: new Date().toISOString().slice(0, 10) });
writeFileSync(join(out, "bench.md"), markdown + "\n");
writeFileSync(join(out, "results.json"), JSON.stringify({ policy, machine, wasmSize, index, verdicts, timings }, null, 1));
console.log(markdown);
console.error(`written: ${join(out, "bench.md")}, ${join(out, "results.json")}`);
