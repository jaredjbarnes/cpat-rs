// What the benchmark times, per corpus file: building its grammars, parsing
// its captured inputs (exec, as the tests ran them), and AutoComplete on the
// truncations of those inputs. Every engine gets the same spec.
import { expandCorpus, loadCorpusFiles } from "./corpus.mjs";

// AutoComplete is timed on at most this many truncations per corpus file,
// spread evenly, so one run of the big grammars stays well under a second.
export const MAX_SUGGEST_CASES = 300;

const spreadOver = (list, count) =>
  list.length <= count ? list : Array.from({ length: count }, (_, i) => list[Math.floor((i * list.length) / count)]);

export function benchSpec() {
  const corpus = expandCorpus(loadCorpusFiles());
  return corpus.files.map(file => {
    const parse = [];
    const suggest = [];
    file.grammars.forEach((grammar, g) => {
      for (const c of grammar.cases) {
        if (c.kind === "captured") parse.push({ g, rule: c.rule, input: c.input });
        if (c.kind === "truncation") suggest.push({ g, rule: c.rule, input: c.input });
      }
    });
    return {
      name: file.name,
      grammars: file.grammars.map(grammar => grammar.text),
      parse,
      suggest: spreadOver(suggest, MAX_SUGGEST_CASES),
      // UTF-16 units, for throughput; the same count for every engine.
      parseUnits: parse.reduce((n, c) => n + c.input.length, 0),
    };
  });
}
