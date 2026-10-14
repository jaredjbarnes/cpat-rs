// Writes corpus/unicode.json, the generated non-ASCII and unusual-whitespace
// corpus (lib/unicode_corpus.mjs), from the captured corpus files. Run it
// after a recapture; test/harness.test.mjs fails while the file is stale.
//
//   node conformance/capture/unicode.mjs
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { loadCorpusFiles } from "../lib/corpus.mjs";
import { corpusDir } from "../lib/paths.mjs";
import { unicodeCorpus } from "../lib/unicode_corpus.mjs";

const corpus = unicodeCorpus(loadCorpusFiles());
writeFileSync(join(corpusDir, "unicode.json"), JSON.stringify(corpus, null, 1) + "\n");
const cases = corpus.grammars.reduce((n, g) => n + g.cases.length, 0);
console.log(`unicode: ${corpus.grammars.length} grammar(s), ${cases} generated case(s)`);
