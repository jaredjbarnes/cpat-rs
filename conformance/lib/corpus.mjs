// The corpus as the engines see it: every grammar with its cases, captured
// ones first, then the mutations of each captured input. Each case runs
// exec; truncations and captured AutoComplete calls also run AutoComplete.
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { corpusDir } from "./paths.mjs";
import { deletions, truncations } from "./mutate.mjs";

const DEFAULT_AUTOCOMPLETE = { greedyPatternNames: [], customTokens: {}, disableDedupe: false };

export function loadCorpusFiles(dir = corpusDir) {
  return readdirSync(dir)
    .filter(f => f.endsWith(".json"))
    .sort()
    .map(f => JSON.parse(readFileSync(join(dir, f), "utf8")));
}

/** `{ files: [{ name, suite, grammars: [{ key, text, options?, cases }] }] }` where
 * each case is `{ id, rule, input, kind, autocomplete }` and `id` is unique
 * across the corpus. */
export function expandCorpus(files) {
  let next = 0;
  return {
    files: files.map(file => ({
      name: file.name,
      suite: file.suite,
      grammars: file.grammars.map((grammar, index) => {
        const seen = new Set();
        const cases = [];
        const add = (rule, input, kind, autocomplete) => {
          const key = JSON.stringify([rule, input, autocomplete]);
          if (seen.has(key)) return;
          seen.add(key);
          cases.push({ id: next++, rule, input, kind, autocomplete });
        };
        for (const c of grammar.cases) add(c.rule, c.input, c.autocomplete ? "autocomplete" : "captured", c.autocomplete ?? null);
        for (const c of grammar.cases) {
          for (const t of truncations(c.input)) add(c.rule, t, "truncation", c.autocomplete ?? DEFAULT_AUTOCOMPLETE);
          for (const d of deletions(c.input)) add(c.rule, d, "deletion", null);
        }
        const options = grammar.options == null ? {} : { options: grammar.options };
        return { key: `${file.name}#${index}`, text: grammar.text, ...options, cases };
      }),
    })),
  };
}
