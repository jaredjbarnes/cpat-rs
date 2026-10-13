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
import { astRows, errorTriple, threw } from "./normalize.mjs";

const [engine, casesPath, outPath, shardArg = "0/1"] = process.argv.slice(2);
const [shard, shards] = shardArg.split("/").map(Number);
const apiPath = { ts: v12Api, wasm: wasmApi }[engine];
if (apiPath == null) throw new Error(`unknown engine ${engine}`);
const api = await import(pathToFileURL(apiPath).href);
const { Grammar, AutoComplete, generateErrorMessage } = api;

const corpus = JSON.parse(readFileSync(casesPath, "utf8"));
const lines = [];

for (const file of corpus.files) {
  for (const grammar of file.grammars) {
    const cases = grammar.cases.filter(c => c.id % shards === shard);
    // Shard 0 reports every build; the others build only what they parse.
    if (cases.length === 0 && shard !== 0) continue;
    let patterns = null;
    let buildError = null;
    try {
      patterns = Grammar.parseString(grammar.text);
    } catch (error) {
      buildError = threw(error);
    }
    if (shard === 0) {
      lines.push(JSON.stringify({ build: grammar.key, error: buildError, rules: patterns == null ? null : Object.keys(patterns) }));
    }
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
    const { ast, cursor } = pattern.exec(input);
    let message;
    try {
      message = generateErrorMessage(pattern, cursor);
    } catch (error) {
      message = { threw: threw(error) };
    }
    return {
      matched: ast != null,
      ast: astRows(ast, input),
      error: errorTriple(cursor.error),
      furthest: errorTriple(cursor.furthestError),
      message,
      // cpat's depth limit (README, DESIGN §11); v12 has none.
      ...(cursor.tooDeep ? { tooDeep: true } : {}),
    };
  } catch (error) {
    return { threw: threw(error) };
  }
}

function autoCompleteCase(made, input) {
  try {
    const suggestion = made().suggestFor(input);
    return {
      complete: suggestion.isComplete,
      at: suggestion.errorAtIndex,
      options: suggestion.options.map(o => [o.text, o.startIndex]),
      ...(suggestion.cursor?.tooDeep ? { tooDeep: true } : {}),
    };
  } catch (error) {
    return { threw: threw(error) };
  }
}
