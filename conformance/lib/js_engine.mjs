// Runs the expanded corpus through a clarity-pattern-parser-shaped API and
// writes what each case observably did, in one normalized shape that the
// native engine (conformance/native) writes too. The same code drives TS
// v12 (under tsx) and cpat-wasm's JS API, so any difference between them
// is the engines', not the harness's.
//
//   node conformance/lib/js_engine.mjs <ts|wasm> <cases.json> <out.jsonl> [i/n]
import { readFileSync, writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { v12Api, wasmApi } from "./paths.mjs";
import { autoCompleteResult, buildResult, execResult, threw } from "./normalize.mjs";
import { replayBuild } from "./replay.mjs";

const [engine, casesPath, outPath, shardArg = "0/1"] = process.argv.slice(2);
const [shard, shards] = shardArg.split("/").map(Number);
const apiPath = { ts: v12Api, wasm: wasmApi }[engine];
if (apiPath == null) throw new Error(`unknown engine ${engine}`);
const api = await import(pathToFileURL(apiPath).href);
const { AutoComplete, generateErrorMessage } = api;

const corpus = JSON.parse(readFileSync(casesPath, "utf8"));
const lines = [];

for (const file of corpus.files) {
  for (const grammar of file.grammars) {
    const cases = grammar.cases.filter(c => c.id % shards === shard);
    // Shard 0 reports every build; the others build only what they parse.
    if (cases.length === 0 && shard !== 0) continue;
    const built = await replayBuild(api, grammar, threw);
    if (shard === 0) {
      lines.push(JSON.stringify(buildResult(grammar.key, built, grammar.options?.decorators != null)));
    }
    const patterns = built.patterns;
    // One AutoComplete per rule and options, reused as an editor would.
    const autoCompletes = new Map();
    for (const c of cases) lines.push(JSON.stringify({ id: c.id, ...runCase(patterns, c, autoCompletes) }));
  }
}

writeFileSync(outPath, lines.join("\n") + "\n");

function runCase(patterns, c, autoCompletes) {
  const pattern = patterns?.[c.rule];
  if (pattern == null) return { missing: true };
  const result = { exec: execCase(pattern, c.input) };
  if (c.autocomplete != null) {
    const key = JSON.stringify([c.rule, c.autocomplete]);
    const made = () => {
      if (!autoCompletes.has(key)) autoCompletes.set(key, new AutoComplete(pattern, c.autocomplete));
      return autoCompletes.get(key);
    };
    result.autocomplete = autoCompleteCase(made, c.input);
  }
  return result;
}

function execCase(pattern, input) {
  try {
    const result = pattern.exec(input);
    let message;
    try {
      message = generateErrorMessage(pattern, result.cursor);
    } catch (error) {
      message = { threw: threw(error) };
    }
    return execResult(result, message, input);
  } catch (error) {
    return { threw: threw(error) };
  }
}

function autoCompleteCase(made, input) {
  try {
    return autoCompleteResult(made().suggestFor(input));
  } catch (error) {
    return { threw: threw(error) };
  }
}
