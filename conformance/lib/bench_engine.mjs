// Times one engine on the bench spec: for each corpus file, `runs` timed
// runs (after `warmup` untimed ones) of building its grammars, parsing its
// captured inputs and suggesting for its truncations. Writes the run times
// in milliseconds. Measurement only: nothing here is asserted.
//
//   node conformance/lib/bench_engine.mjs <ts|wasm> <spec.json> <out.json> <warmup> <runs>
import { readFileSync, writeFileSync } from "node:fs";
import { pathToFileURL } from "node:url";
import { v12Api, wasmApi } from "./paths.mjs";

const [engine, specPath, outPath, warmupArg, runsArg] = process.argv.slice(2);
const api = await import(pathToFileURL({ ts: v12Api, wasm: wasmApi }[engine]).href);
const { Grammar, AutoComplete } = api;
const spec = JSON.parse(readFileSync(specPath, "utf8"));
const warmup = Number(warmupArg);
const runs = Number(runsArg);

const build = file =>
  file.grammars.map(text => {
    try {
      return Grammar.parseString(text);
    } catch {
      return null;
    }
  });

// A throw (v12's stack overflow on jq's paren bomb) is part of the work.
const attempt = work => {
  try {
    work();
  } catch {}
};

function time(work) {
  for (let i = 0; i < warmup; i++) work();
  const times = [];
  for (let i = 0; i < runs; i++) {
    const start = performance.now();
    work();
    times.push(performance.now() - start);
  }
  return times;
}

const results = spec.map(file => {
  const built = build(file);
  const autoCompletes = new Map();
  const autoComplete = c => {
    const key = `${c.g}\0${c.rule}`;
    if (!autoCompletes.has(key)) autoCompletes.set(key, new AutoComplete(built[c.g][c.rule]));
    return autoCompletes.get(key);
  };
  return {
    name: file.name,
    build: time(() => build(file)),
    parse: time(() => {
      for (const c of file.parse) attempt(() => built[c.g]?.[c.rule]?.exec(c.input));
    }),
    suggest: time(() => {
      for (const c of file.suggest) attempt(() => autoComplete(c).suggestFor(c.input));
    }),
  };
});

writeFileSync(outPath, JSON.stringify(results));
