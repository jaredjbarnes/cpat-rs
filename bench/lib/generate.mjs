// Writes every workload the benchmark runs, one JSON file each, plus an
// index. Runs under tsx with TS v12, which picks the realistic inputs: a
// captured test input joins a big document only if v12 matches it, alone
// and between two neighbours. (Whether cpat-rs agrees is checked later, by
// the verify step; generating with v12 only decides what "realistic" is.)
//
//   npx tsx bench/lib/generate.mjs <out dir>
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { corpusDir, v12Api } from "../../conformance/lib/paths.mjs";
import {
  arithFlat,
  arithNested,
  asciiTwinText,
  jsonText,
  jsonTextOfSize,
  keywordsGrammar,
  keywordsText,
  nonAsciiText,
  syntheticGrammar,
  tokensText,
} from "./synthetic.mjs";
import { MB, repeatTo, spreadOver, utf8Bytes } from "./text.mjs";

const out = process.argv[2];
if (out == null) {
  console.error("usage: npx tsx bench/lib/generate.mjs <out dir>");
  process.exit(2);
}
const { Grammar } = await import(pathToFileURL(v12Api).href);
const benchDir = join(dirname(fileURLToPath(import.meta.url)), "..");
const grammarFile = name => readFileSync(join(benchDir, "grammars", `${name}.cpat`), "utf8");
const corpus = name => JSON.parse(readFileSync(join(corpusDir, `${name}.json`), "utf8")).grammars[0];

// The agent-harness grammars as the harness builds them (the corpus holds
// the text each test built), and the rule its tests parse with.
const HARNESS = ["shell", "jq", "saya", "sed", "calc", "find", "glob", "date-expr", "regex-bre", "regex-ere", "regex-pcre"];

// Captured inputs longer than this stay out of the realistic workloads: the
// shell corpus holds one 500 KB string literal (a backtracking-limit test),
// which would make a batch a benchmark of one regex.
const MAX_CAPTURED = 4000;

const workloads = [];
const add = workload => {
  const inputs = workload.cases ?? [];
  const reps = workload.reps ?? 1;
  workloads.push({
    ...workload,
    reps,
    cases: inputs,
    // Per iteration, for throughput.
    units: reps * (workload.kind === "build" ? workload.grammar.length : inputs.reduce((n, [, t]) => n + t.length, 0)),
    bytes: reps * (workload.kind === "build" ? utf8Bytes(workload.grammar) : inputs.reduce((n, [, t]) => n + utf8Bytes(t), 0)),
  });
};

const matches = (pattern, text) => {
  try {
    return pattern.exec(text).ast != null;
  } catch {
    return false;
  }
};

/** Captured inputs on `rule` that v12 matches, unique, in corpus order,
 * without the ones `exclude` names. */
function matchingInputs(name, rule, exclude = () => false) {
  const grammar = corpus(name);
  const pattern = Grammar.parseString(grammar.text)[rule];
  const seen = new Set();
  return grammar.cases
    .filter(c => c.rule === rule && c.input.trim().length > 0 && c.input.length <= MAX_CAPTURED && !exclude(c.input))
    .map(c => c.input)
    .filter(input => !seen.has(input) && seen.add(input) && matches(pattern, input));
}

/** A document of about `bytes` made of captured inputs joined by
 * `separator`, keeping only inputs that still match with a neighbour on
 * each side (`probe` stands in for one). */
function realisticDocument(name, rule, separator, probe, bytes, exclude) {
  const pattern = Grammar.parseString(corpus(name).text)[rule];
  const fits = matchingInputs(name, rule, exclude).filter(input => matches(pattern, `${probe}${separator}${input}${separator}${probe}`));
  const unit = fits.join(separator);
  if (!matches(pattern, unit)) throw new Error(`${name}: the joined unit of ${fits.length} inputs doesn't match`);
  const text = repeatTo(unit, separator, bytes);
  if (!matches(pattern, text)) throw new Error(`${name}: the ${bytes}-byte document doesn't match`);
  return { text, inputs: fits.length };
}

// ── (a) build ──────────────────────────────────────────────────────────
for (const name of HARNESS) {
  add({ name: `build/${name}`, group: "build", kind: "build", grammar: corpus(name).text, title: `agent-harness ${name}.cpat` });
}
add({ name: "build/typescript", group: "build", kind: "build", grammar: corpus("v12-typescript").text, title: "v12's TypeScript grammar (typescript.test.ts)" });
add({ name: "build/synthetic-1200", group: "build", kind: "build", grammar: syntheticGrammar(1200), title: "generated: 7 200 rules + an expression" });

// ── (b) parse: realistic documents and the harness's own test inputs ──
// A backtick substitution followed by more script costs time in proportion
// to everything after it, in v12 (4 test inputs made a 136 KB document run
// v12 out of a 4 GB heap). They stay out of the throughput document and get
// a workload of their own (micro/shell-backticks).
const backtick = input => input.includes("`");
{
  const shell = realisticDocument("shell", "script", "\n", "true", MB, backtick);
  add({ name: "parse/shell-1MB", group: "parse", kind: "parse", rule: "script", grammar: corpus("shell").text, cases: [["script", shell.text]], title: `1 MB shell script (${shell.inputs} test scripts, repeated)` });
  const saya = realisticDocument("saya", "statements", "\n", "let probe = 1;", MB);
  add({ name: "parse/saya-1MB", group: "parse", kind: "parse", grammar: corpus("saya").text, cases: [["statements", saya.text]], title: `1 MB saya program (${saya.inputs} test programs, repeated)` });
  // jq has no statement list: a big program is one expression. Joining
  // filters with `,` would make one infix chain as long as the program,
  // which overflows v12's stack, so the test filters, parenthesized, become
  // the values of one object construction (a repeat in the grammar).
  const jqPattern = Grammar.parseString(corpus("jq").text).filter;
  const filters = matchingInputs("jq", "filter")
    .map(f => `(${f})`)
    .filter(f => matches(jqPattern, `{a: ${f}, b: .}`));
  const entries = [];
  for (let size = 0, i = 0; size < MB / 4; i++) {
    const entry = `k${i}: ${filters[i % filters.length]}`;
    entries.push(entry);
    size += utf8Bytes(entry) + 2;
  }
  const jqText = `{\n${entries.join(",\n")}\n}`;
  if (!matches(jqPattern, jqText)) throw new Error("jq: the 256 KB program doesn't match");
  add({ name: "parse/jq-256KB", group: "parse", kind: "parse", grammar: corpus("jq").text, cases: [["filter", jqText]], title: `256 KB jq program: one object whose values are the ${filters.length} test filters, repeated` });
}
for (const name of [...HARNESS, "v12-typescript"]) {
  const grammar = corpus(name);
  const patterns = Grammar.parseString(grammar.text);
  const seen = new Set();
  // An input v12 throws on can't be compared: jq's 3000-deep paren bomb
  // overflows v12's stack where cpat-rs stops with tooDeep (DESIGN §11).
  const throws = ([rule, input]) => {
    try {
      patterns[rule].exec(input);
      return false;
    } catch {
      return true;
    }
  };
  const unique = grammar.cases
    .filter(c => c.input.length <= MAX_CAPTURED)
    .map(c => [c.rule, c.input])
    .filter(([rule, input]) => !seen.has(`${rule}\0${input}`) && seen.add(`${rule}\0${input}`));
  const cases = unique.filter(c => !throws(c));
  const short = name === "v12-typescript" ? "typescript" : name;
  const left = unique.length - cases.length;
  const leftOut = left === 0 ? "" : ` (${left} v12 throws on left out)`;
  add({ name: `tests/${short}`, group: "tests", kind: "parse", grammar: grammar.text, cases, title: `every captured ${short} test input, matching or not: ${cases.length}${leftOut}` });
}

// ── (c) micro ──────────────────────────────────────────────────────────
add({ name: "micro/literal-choice-1MB", group: "micro", kind: "parse", grammar: keywordsGrammar(), cases: [["keywords", keywordsText(MB)]], title: "1 MB of JS keywords: a 64-literal choice in a divided repeat" });
add({ name: "micro/regex-tokens-1MB", group: "micro", kind: "parse", grammar: grammarFile("tokens"), cases: [["tokens", tokensText(MB)]], title: "1 MB of code through a 6-regex token choice" });
// v12 overflows its stack on a flat chain of 64 KB; 16 KB parses.
add({ name: "micro/pratt-flat-16KB", group: "micro", kind: "parse", grammar: grammarFile("arith"), cases: [["expr", arithFlat(16 * 1024)]], reps: 8, title: "16 KB flat operator chain (1+2*3-…) through the Pratt choice, ×8" });
add({ name: "micro/pratt-nested-200", group: "micro", kind: "parse", grammar: grammarFile("arith"), cases: [["expr", arithNested(200)]], reps: 50, title: "200 nested parentheses with an operator each, ×50" });
add({ name: "micro/json-small", group: "micro", kind: "parse", grammar: grammarFile("json"), cases: [["json", jsonText(6, 5)]], reps: 500, title: `a ${utf8Bytes(jsonText(6, 5))}-byte JSON document, ×500` });
const json1MB = jsonTextOfSize(MB, 6);
add({ name: "micro/json-1MB", group: "micro", kind: "parse", grammar: grammarFile("json"), cases: [["json", json1MB]], title: "1 MB JSON document" });
// Error path: the same megabyte, with its closing bracket gone, so the parse
// fails at the very end and every enclosing pattern unwinds.
add({ name: "micro/json-1MB-fail-at-end", group: "micro", kind: "parse", grammar: grammarFile("json"), cases: [["json", json1MB.slice(0, json1MB.lastIndexOf("]"))]], title: "1 MB JSON missing its final ]" });
{
  const shell = realisticDocument("shell", "script", "\n", "true", MB, backtick).text;
  add({ name: "micro/shell-1MB-fail-at-end", group: "micro", kind: "parse", grammar: corpus("shell").text, cases: [["script", `${shell}\necho "unterminated`]], title: "the 1 MB shell script + an unterminated string" });
}
// Each `echo \`…\`` line multiplies the shell grammar's work, in v12 and
// cpat-rs alike (exponential backtracking in the grammar: 8 lines take v12
// about 35 s), so this stays at 6 lines.
add({ name: "micro/shell-backticks-6", group: "micro", kind: "parse", grammar: corpus("shell").text, cases: [["script", Array.from({ length: 6 }, (_, i) => `echo \`echo line${i}\``).join("\n")]], title: "6 lines of echo `echo …`: the shell grammar backtracks exponentially" });
add({ name: "micro/non-ascii-1MB", group: "micro", kind: "parse", grammar: grammarFile("words"), cases: [["text", nonAsciiText(MB)]], title: "1 MB of mixed-script words, emoji and combining marks" });
add({ name: "micro/ascii-twin-1MB", group: "micro", kind: "parse", grammar: grammarFile("words"), cases: [["text", asciiTwinText(MB)]], title: "the same text shape in ASCII only (control for the above)" });

// ── (d) AutoComplete on partial inputs ─────────────────────────────────
/** 40 test inputs between 12 and 400 units, each cut at 60%: an editor's
 * half-typed line. */
function partials(name, rule) {
  const inputs = matchingInputs(name, rule).filter(input => input.length >= 12 && input.length <= 400);
  return spreadOver(inputs, 40).map(input => [rule, input.slice(0, Math.floor(input.length * 0.6))]);
}
for (const [name, rule] of [["shell", "script"], ["jq", "filter"], ["saya", "statements"], ["calc", "calc"]]) {
  const cases = partials(name, rule);
  add({ name: `suggest/${name}`, group: "suggest", kind: "suggest", grammar: corpus(name).text, cases, title: `suggestFor on ${cases.length} half-typed ${name} test inputs` });
}
add({ name: "suggest/json", group: "suggest", kind: "suggest", grammar: grammarFile("json"), cases: [["json", jsonText(6, 5).slice(0, 360)]], reps: 20, title: "suggestFor on a small JSON document cut at 60% (360 units), ×20" });

// ── baseline: load the engine, do nothing (for memory) ─────────────────
add({ name: "noop", group: "noop", kind: "noop", grammar: "", title: "engine loaded, nothing run (memory baseline)" });

mkdirSync(join(out, "workloads"), { recursive: true });
const index = workloads.map((w, i) => {
  const file = `${String(i).padStart(2, "0")}-${w.name.replace(/\//g, "--")}.json`;
  writeFileSync(join(out, "workloads", file), JSON.stringify(w));
  const { grammar, cases, ...meta } = w;
  return { ...meta, file, inputs: cases.length };
});
writeFileSync(join(out, "index.json"), JSON.stringify(index, null, 1));
for (const w of index) console.error(`${w.name.padEnd(30)} ${String(w.inputs).padStart(5)} inputs × ${w.reps}  ${(w.bytes / 1024).toFixed(1)} KB`);
