// The conformance harness: runs the corpus through TS v12, cpat-rs native
// and cpat-wasm, compares both cpat engines with v12 and reports every
// difference. Exits 0 only when every difference is explained by
// conformance/allowlist.json (and no entry there is stale).
//
//   node conformance/run.mjs            build, run, compare, report
//   node conformance/run.mjs --no-build use the existing release builds
//   node conformance/run.mjs --perturb  shift one native AST range on purpose
//   node conformance/run.mjs --self-test  prove a perturbed run fails
//   node conformance/run.mjs --examples N  examples printed per group (5)
//
// The report is printed and written to target/conformance/report.md.
import { spawn, execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { availableParallelism } from "node:os";
import { join } from "node:path";
import { applyAllowlist, loadAllowlist } from "./lib/allowlist.mjs";
import { compare, indexResults } from "./lib/compare.mjs";
import { expandCorpus, loadCorpusFiles } from "./lib/corpus.mjs";
import { conformanceDir, nativeBin, repoDir, v12Dir, wasmDir, workDir } from "./lib/paths.mjs";
import { formatReport, summaryLine } from "./lib/report.mjs";

const args = process.argv.slice(2);
const flag = name => args.includes(name);
const option = (name, fallback) => {
  const at = args.indexOf(name);
  return at < 0 ? fallback : args[at + 1];
};

async function main({ build, perturb, examples }) {
  mkdirSync(workDir, { recursive: true });
  if (build) buildEngines();

  const corpus = expandCorpus(loadCorpusFiles());
  const casesPath = join(workDir, "cases.json");
  writeFileSync(casesPath, JSON.stringify(corpus));

  const shards = Math.max(1, Math.min(8, availableParallelism() - 1));
  const engineRuns = await Promise.all([
    runSharded("ts", casesPath, shards),
    runSharded("wasm", casesPath, shards),
    runNative(casesPath, perturb),
  ]);
  const [ts, wasm, native] = engineRuns.map(indexResults);

  const allowlist = loadAllowlist();
  const byNative = compare(corpus, ts, native, "native");
  const byWasm = compare(corpus, ts, wasm, "wasm");
  const { explained, unexplained, stale } = applyAllowlist(allowlist, [...byNative.differences, ...byWasm.differences]);

  const report = {
    perFile: perFile(corpus),
    totals: byNative.counts,
    engines: ["native", "wasm"],
    explained,
    unexplained,
    stale,
    examples,
  };
  const markdown = formatReport(report);
  writeFileSync(join(workDir, "report.md"), markdown + "\n");
  writeFileSync(join(workDir, "differences.json"), JSON.stringify({ explained, unexplained }, null, 1));
  console.log(markdown);
  console.log(`\n${summaryLine(report)}`);
  console.log(`report: ${join(workDir, "report.md")}`);

  const ok = unexplained.length === 0 && stale.length === 0;
  console.log(ok ? "CONFORMANCE PASSED" : "CONFORMANCE FAILED");
  return ok ? 0 : 1;
}

/** A run with one native range shifted must fail, naming that difference. */
async function selfTest() {
  const status = await main({ build: !flag("--no-build"), perturb: true, examples: 3 });
  const { unexplained } = JSON.parse(readFileSync(join(workDir, "differences.json"), "utf8"));
  const caught =
    status !== 0 &&
    unexplained.length === 1 &&
    unexplained[0].kind === "exec.ast" &&
    unexplained[0].engine === "native";
  console.log(caught ? "SELF-TEST PASSED: the shifted range is an unexplained difference" : "SELF-TEST FAILED");
  return caught ? 0 : 1;
}

function buildEngines() {
  const env = { ...process.env, PATH: `${process.env.HOME}/.cargo/bin:${process.env.PATH}` };
  execFileSync("cargo", ["build", "--release", "-p", "cpat-conformance"], { cwd: repoDir, stdio: "inherit", env });
  execFileSync("wasm-pack", ["build", "--target", "nodejs", "--release", "--out-dir", "pkg-node"], {
    cwd: wasmDir,
    stdio: "inherit",
    env,
  });
}

function perFile(corpus) {
  return corpus.files.map(file => {
    const cases = file.grammars.flatMap(g => g.cases);
    return {
      name: file.name,
      suite: file.suite,
      grammars: file.grammars.length,
      captured: cases.filter(c => c.kind === "captured" || c.kind === "autocomplete").length,
      cases: cases.length,
      suggestions: cases.filter(c => c.autocomplete != null).length,
    };
  });
}

function run(command, commandArgs, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, commandArgs, { cwd, stdio: ["ignore", "inherit", "inherit"] });
    child.on("error", reject);
    child.on("exit", code => (code === 0 ? resolve() : reject(new Error(`${command} ${commandArgs.join(" ")} exited ${code}`))));
  });
}

const readLines = path =>
  readFileSync(path, "utf8")
    .split("\n")
    .filter(Boolean)
    .map(line => JSON.parse(line));

/** TS v12 runs under tsx against v12's src; wasm under plain node. */
async function runSharded(engine, casesPath, shards) {
  const script = join(conformanceDir, "lib", "js_engine.mjs");
  const outs = Array.from({ length: shards }, (_, i) => join(workDir, `results-${engine}-${i}.jsonl`));
  await Promise.all(
    outs.map((out, i) => {
      const engineArgs = [script, engine, casesPath, out, `${i}/${shards}`];
      return engine === "ts"
        ? run("npx", ["--yes", "tsx", ...engineArgs], v12Dir)
        : run(process.execPath, engineArgs, repoDir);
    })
  );
  return outs.flatMap(readLines);
}

async function runNative(casesPath, perturb) {
  const out = join(workDir, "results-native.jsonl");
  await run(nativeBin, [casesPath, out, ...(perturb ? ["--perturb"] : [])], repoDir);
  return readLines(out);
}

if (flag("--self-test")) {
  process.exit(await selfTest());
}
process.exit(await main({ build: !flag("--no-build"), perturb: flag("--perturb"), examples: Number(option("--examples", 5)) }));
