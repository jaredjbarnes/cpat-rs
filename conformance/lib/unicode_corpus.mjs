// The generated corpus of non-ASCII and unusual-whitespace inputs
// (corpus/unicode.json, written by capture/unicode.mjs). Real tests mostly
// run ASCII; this is where JS's own ideas of a character, a space and a
// line end meet cpat-rs's: `.`, `\s`, `\b` and `[^…]` in the grammars'
// regexes, grapheme steps, UTF-16 positions and a surrogate pair at the end.
// Built from the captured corpus alone, so the same corpus always gives the
// same file (test/harness.test.mjs checks the committed one).
import { spread } from "./mutate.mjs";

/** JS whitespace and line terminators, and two characters that look like
 * space but aren't (`\u0085` is no `\s` in JS; `​` is no space at all). */
export const SPACES = [
  "\r", "\r\n", "\t", "\u000B", "\u000C", " ", "\u0085", " ",
  "​", " ", " ", " ", "　", "﻿",
];

/** Words that aren't ASCII: precomposed and combining accents, a skin tone,
 * a ZWJ family, CJK, a letter outside the BMP, case-mapping oddities. */
export const WORDS = [
  "é", "é", "👍🏽", "👨‍👩‍👧", "漢字", "𝐀", "İ", "ß", "ﬁ", "Ω", "١٢٣",
];

/** What may end an input: surrogate pairs, a dangling mark, a BOM, a CR. */
export const ENDINGS = ["😀", "𝒳", "👍🏽", "́", "﻿", "\r"];

// Seeds per agent-harness grammar, and their longest input: each variant
// is mutated (corpus.mjs), so a few short seeds already give thousands of
// cases.
const SEEDS_PER_GRAMMAR = 4;
const MAX_SEED_LENGTH = 60;

/** Rules whose regexes JS and Rust read differently if the translation
 * slips (DESIGN §6), each run on every probe below. */
export const REGEX_GRAMMAR = `
dot = /./
dots = /.+/
spaces = /\\s+/
non-spaces = /\\S+/
words = /\\w+/
non-words = /\\W+/
bounded-word = /\\b\\w+\\b/
digits = /\\d+/
not-newline = /[^\\n]+/
line = /[^\\r\\n\\u2028\\u2029]*/
line-end = /\\r\\n|\\r|\\n|\\u2028|\\u2029/
letters = /\\p{L}+/
letter-marks = /(\\p{L}\\p{M}*)+/
pictographs = /\\p{Extended_Pictographic}+/
anything = /[\\s\\S]+/
token = /[^\\s"]+/
quoted = /"[^"]*"/
lines = (line, line-end)+
spaced-words = (non-spaces, spaces)+
text = (quoted | token | spaces)+
`;

/** Characters that join the one before them into a single grapheme: a
 * combining mark, a skin-tone modifier, a ZWJ and a variation selector. */
export const JOINERS = ["\u0301", "\u{1F3FD}", "\u200D", "\uFE0F"];

/** Literals and a Pratt expression, for a joiner after a character a literal
 * matches alone: the literal ends inside the grapheme, the next leaf starts
 * after it, and the joiner belongs to no node. Both exec and AutoComplete
 * must then call the text incomplete (v12 compares the leaves' joined
 * values with the text), wherever the skip falls: first, between two
 * leaves, inside a repeat, after an operator. */
export const SPLIT_GRAMMAR = `
a = "a"
b = "b"
pair = a + b
as = (a)+
listed = (a, ",")+
num = /\\d+/
add = expr + "+" + expr
neg = "-" + expr
inc = expr + "++"
expr = add | neg | inc | num
`;

/** Each rule's inputs, before a joiner goes after each character. */
export const SPLIT_SEEDS = {
  pair: ["ab"],
  as: ["aaa"],
  listed: ["a,a"],
  expr: ["1+2", "-1++", "1+-2"],
};

/** Every seed, then the seed with each joiner after each of its characters. */
export function splitProbes(seed) {
  const chars = [...seed];
  const out = [seed];
  for (let at = 1; at <= chars.length; at++) {
    for (const joiner of JOINERS) out.push(chars.slice(0, at).join("") + joiner + chars.slice(at).join(""));
  }
  return out;
}

/** Inputs for REGEX_GRAMMAR's rules: each space alone and between words,
 * the words, the endings after a word, and a few mixed lines. */
export function regexProbes() {
  return [
    ...SPACES,
    ...SPACES.map(space => `a${space}b`),
    ...WORDS,
    ...ENDINGS.map(ending => `ab${ending}`),
    "a\r\nb\nc d e",
    "﻿word \"quoted é\" 漢字",
    "naïve café é 👨‍👩‍👧 end😀",
  ];
}

/** The variants of one captured input: every space swapped for each
 * unusual one, a non-ASCII word at the first space (or, with none, in the
 * middle), each ending, and a leading BOM. */
export function variants(input) {
  const out = [];
  if (input.includes(" ")) {
    for (const space of SPACES) out.push(input.replaceAll(" ", space));
  }
  const chars = [...input];
  const at = input.includes(" ") ? input.indexOf(" ") + 1 : chars.slice(0, chars.length >> 1).join("").length;
  for (const word of WORDS) out.push(input.slice(0, at) + word + input.slice(at));
  for (const ending of ENDINGS) out.push(input + ending);
  out.push(`﻿${input}`);
  return out;
}

/** A few short captured inputs, spread over the rules. */
function seeds(grammar) {
  const byInput = new Map();
  for (const c of grammar.cases) {
    if (c.input.length > 0 && c.input.length <= MAX_SEED_LENGTH && !byInput.has(c.input)) byInput.set(c.input, c);
  }
  const sorted = [...byInput.values()].sort((a, b) => (a.rule + "\0" + a.input < b.rule + "\0" + b.input ? -1 : 1));
  return spread(sorted, SEEDS_PER_GRAMMAR);
}

/** AutoComplete with v12's defaults. Every generated case runs it on the
 * whole input as well as exec: truncations alone never put a skipped mark
 * between two leaves, and that is where `isComplete` and exec once
 * disagreed in cpat-rs. */
export const DEFAULT_AUTOCOMPLETE = { greedyPatternNames: [], customTokens: {}, disableDedupe: false };

/** The corpus file: REGEX_GRAMMAR with its probes, SPLIT_GRAMMAR with
 * joiners after each character, and each agent-harness grammar with the
 * variants of its seeds. Every case also runs AutoComplete. */
export function unicodeCorpus(files) {
  const rules = [...REGEX_GRAMMAR.matchAll(/^([a-z-]+) =/gm)].map(m => m[1]);
  const probes = regexProbes();
  const grammars = [
    {
      id: "regex-probes",
      origin: "generated: unicode_corpus.mjs REGEX_GRAMMAR",
      text: REGEX_GRAMMAR,
      cases: rules.flatMap(rule => probes.map(input => ({ rule, input, origin: "generated: regex probe" }))),
    },
    {
      id: "split-graphemes",
      origin: "generated: unicode_corpus.mjs SPLIT_GRAMMAR",
      text: SPLIT_GRAMMAR,
      cases: Object.entries(SPLIT_SEEDS).flatMap(([rule, seeds]) =>
        seeds.flatMap(seed => splitProbes(seed).map(input => ({ rule, input, origin: "generated: joiner after a character" }))),
      ),
    },
  ];
  for (const file of files.filter(f => f.suite === "agent-harness")) {
    for (const grammar of file.grammars) {
      const cases = [];
      const seen = new Set();
      for (const seed of seeds(grammar)) {
        for (const input of variants(seed.input)) {
          const key = `${seed.rule}\0${input}`;
          if (seen.has(key)) continue;
          seen.add(key);
          cases.push({ rule: seed.rule, input, origin: `generated: variant of ${JSON.stringify(seed.input)}` });
        }
      }
      grammars.push({ id: `${file.name}-unicode`, origin: `generated from corpus/${file.name}.json`, text: grammar.text, cases });
    }
  }
  for (const grammar of grammars) {
    grammar.cases = grammar.cases.map(c => ({ ...c, autocomplete: DEFAULT_AUTOCOMPLETE }));
  }
  return { name: "unicode", suite: "generated", grammars };
}
