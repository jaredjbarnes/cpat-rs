// Turns capture logs into conformance/corpus: one file per agent-harness
// grammar and one per v12 test file, each case de-duplicated and sorted so
// a recapture only changes what really changed.
import { readdirSync, readFileSync, rmSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { corpusDir } from "../lib/paths.mjs";

/** agent-harness's grammar files by a short corpus name. */
export function harnessName(file) {
  return file
    .replace(/\.cpat$/, "")
    .replace(/\/grammar$/, "")
    .replace("shell/regex/grammars/", "regex-")
    .replace("shell/glob", "glob")
    .replace("tools/", "")
    .replaceAll("_", "-");
}

/** v12's test files by a short corpus name. */
function v12Name(origin) {
  const file = origin.split(" :: ")[0].split("/").pop();
  const base = file.replace(/\.test\.ts$/, "");
  return base === "typescript" ? "v12-typescript" : `v12-${base.toLowerCase()}-test`;
}

function readLogs(dirs) {
  const records = [];
  for (const dir of dirs) {
    for (const file of readdirSync(dir).sort()) {
      for (const line of readFileSync(join(dir, file), "utf8").split("\n")) {
        if (line) records.push(JSON.parse(line));
      }
    }
  }
  return records;
}

const byText = (a, b) => (a < b ? -1 : a > b ? 1 : 0);

export function assemble(dirs) {
  const records = readLogs(dirs);
  const grammars = new Map();
  for (const r of records) {
    if (r.t !== "grammar") continue;
    const known = grammars.get(r.g);
    // The earliest origin, so the choice doesn't depend on worker order.
    if (known == null || byText(r.origin, known.origin) < 0) grammars.set(r.g, r);
  }

  const files = new Map();
  for (const g of grammars.values()) {
    // agent-harness tests build ad-hoc grammars too; only its eleven files count.
    if (g.source === "agent-harness" && g.file == null) continue;
    const name = g.source === "agent-harness" ? harnessName(g.file) : v12Name(g.origin);
    const entry = files.get(name) ?? {
      name,
      suite: g.source,
      ...(g.file != null ? { file: g.file } : {}),
      grammars: new Map(),
    };
    entry.grammars.set(g.g, { id: g.g, text: g.text, origin: g.origin, cases: new Map() });
    files.set(name, entry);
  }

  const grammarFile = new Map();
  for (const entry of files.values()) for (const id of entry.grammars.keys()) grammarFile.set(id, entry);

  for (const r of records) {
    if (r.t === "grammar") continue;
    const entry = grammarFile.get(r.g);
    if (entry == null) continue;
    const grammar = entry.grammars.get(r.g);
    const autocomplete = r.t === "ac" ? r.options : null;
    const key = JSON.stringify([r.rule, r.input, autocomplete]);
    const known = grammar.cases.get(key);
    if (known == null || byText(r.origin, known.origin) < 0) {
      grammar.cases.set(key, { rule: r.rule, input: r.input, ...(autocomplete ? { autocomplete } : {}), origin: r.origin });
    }
  }

  rmSync(corpusDir, { recursive: true, force: true });
  mkdirSync(corpusDir, { recursive: true });
  for (const entry of [...files.values()].sort((a, b) => byText(a.name, b.name))) {
    const out = {
      name: entry.name,
      suite: entry.suite,
      ...(entry.file ? { file: entry.file } : {}),
      grammars: [...entry.grammars.values()]
        .sort((a, b) => byText(a.origin + a.id, b.origin + b.id))
        .map(g => ({
          id: g.id,
          origin: g.origin,
          text: g.text,
          cases: [...g.cases.values()].sort(
            (a, b) => byText(a.rule, b.rule) || byText(a.input, b.input) || byText(JSON.stringify(a.autocomplete ?? null), JSON.stringify(b.autocomplete ?? null))
          ),
        })),
    };
    writeFileSync(join(corpusDir, `${entry.name}.json`), JSON.stringify(out, null, 1) + "\n");
    const cases = out.grammars.reduce((n, g) => n + g.cases.length, 0);
    console.log(`${entry.name}: ${out.grammars.length} grammar(s), ${cases} captured case(s)`);
  }
}

if (process.argv[1] && import.meta.url.endsWith(process.argv[1].split("/").pop())) {
  assemble(process.argv.slice(2));
}
