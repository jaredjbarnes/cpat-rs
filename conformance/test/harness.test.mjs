// The harness's own logic, on small made-up results: a shifted range is a
// difference, every compared field is normalized and compared (dropping
// one fails), the allowlist explains only what it names, a stale entry is
// reported, captured options replay from their record, and mutations and
// the generated corpus are what the corpus says they are.
//
//   node --test conformance/test/*.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { applyAllowlist, loadAllowlist } from "../lib/allowlist.mjs";
import { compare, indexResults } from "../lib/compare.mjs";
import { loadCorpusFiles } from "../lib/corpus.mjs";
import { grammarUses } from "../lib/features.mjs";
import { deletions, tokenBoundaries, truncations } from "../lib/mutate.mjs";
import { autoCompleteResult, buildResult, execResult } from "../lib/normalize.mjs";
import { corpusDir } from "../lib/paths.mjs";
import { replayBuild, replayOptions } from "../lib/replay.mjs";
import { SPACES, unicodeCorpus, variants } from "../lib/unicode_corpus.mjs";

const corpus = {
  files: [{ name: "calc", grammars: [{ key: "calc#0", cases: [{ id: 0, rule: "expr", input: "1+2", kind: "captured" }] }] }],
};
const build = { build: "calc#0", error: null, rules: ["expr"] };
const exec = ast => ({
  matched: true,
  ast,
  error: null,
  furthest: [3, 3, "num"],
  message: "Error at line 1, column 3. Hint: ",
});
const ts = indexResults([build, { id: 0, exec: exec("0|expression|add|0|3\n1|regex|num|0|1") }]);

test("equal results have no differences, and every field is counted", () => {
  const { counts, differences } = compare(corpus, ts, ts, "native");
  assert.deepEqual(differences, []);
  assert.deepEqual(counts, { cases: 1, builds: 1, matches: 1, asts: 1, errors: 1, messages: 1, suggestions: 0 });
});

test("a range shifted by one is an exec.ast difference", () => {
  const shifted = indexResults([build, { id: 0, exec: exec("0|expression|add|0|4\n1|regex|num|0|1") }]);
  const { differences } = compare(corpus, ts, shifted, "native");
  assert.deepEqual(
    differences.map(d => [d.kind, d.engine, d.id]),
    [["exec.ast", "native", 0]]
  );
});

test("a throw is one difference, not one per field", () => {
  const threw = indexResults([build, { id: 0, exec: { threw: "RangeError: boom" } }]);
  const { differences } = compare(corpus, ts, threw, "wasm");
  assert.deepEqual(differences.map(d => d.kind), ["exec.threw"]);
});

test("a missing result and a different build are differences", () => {
  const nothing = indexResults([{ build: "calc#0", error: "Error: no", rules: null }]);
  const { differences } = compare(corpus, ts, nothing, "wasm");
  assert.deepEqual(differences.map(d => d.kind), ["build.error", "case.lost"]);
});

test("the allowlist explains only the differences it names, and reports stale entries", () => {
  const shifted = indexResults([build, { id: 0, exec: exec("0|expression|add|0|4\n1|regex|num|0|1") }]);
  const { differences } = compare(corpus, ts, shifted, "native");
  const entries = [
    { id: "other-file", kind: "exec.ast", file: "jq", reason: "r", cites: "DESIGN.md §1" },
    { id: "stale", kind: "exec.threw", reason: "r", cites: "DESIGN.md §1" },
  ];
  const first = applyAllowlist(entries, differences);
  assert.equal(first.unexplained.length, 1);
  assert.deepEqual(first.stale.map(e => e.id), ["other-file", "stale"]);

  const named = [{ id: "calc-ast", kind: "exec.ast", file: "calc", engines: ["native"], reason: "r", cites: "DESIGN.md §1" }];
  const second = applyAllowlist(named, differences);
  assert.equal(second.unexplained.length, 0);
  assert.deepEqual(second.explained.map(d => d.allowlist), ["calc-ast"]);
});

test("an entry can require more of the result than the differing field", () => {
  const differences = [{ kind: "exec.threw", engine: "native", expected: "RangeError: x", actual: null, got: { tooDeep: true } }];
  const entry = { id: "deep", kind: "exec.threw", actual: null, got: { tooDeep: true }, reason: "r", cites: "DESIGN.md §11" };
  assert.equal(applyAllowlist([entry], differences).unexplained.length, 0);
  const shallow = [{ ...differences[0], got: {} }];
  assert.equal(applyAllowlist([entry], shallow).unexplained.length, 1);
});

test("tokens end between words and spaces and around punctuation", () => {
  assert.deepEqual(tokenBoundaries("ab  c(d)"), [2, 4, 5, 6, 7]);
  assert.deepEqual(truncations("ab c"), ["", "ab", "ab "]);
  // An emoji is one character: a cut never splits its surrogate pair.
  assert.deepEqual(truncations("a😀b"), ["", "a", "a😀"]);
  assert.deepEqual(deletions("ab c"), ["b c", "abc", "ab "]);
  assert.deepEqual(deletions("😀b"), ["b", "😀"]);
});

// ─── Every compared field is reported and compared ──────────────────────
// The observables, written out here rather than read from compare.mjs, so
// dropping one from normalize.mjs or compare.mjs fails a test below.

const OBSERVED = {
  build: ["error", "rules", "decorated"],
  exec: ["matched", "ast", "error", "furthest", "message", "tooDeep"],
  autocomplete: ["complete", "at", "options", "tooDeep"],
};

// v12-shaped results with every observable set to something no default
// would give.
const leaf = { type: "literal", name: "a", startIndex: 0, endIndex: 1, value: "a", children: [] };
const v12Exec = {
  ast: { type: "sequence", name: "s", startIndex: 0, endIndex: 1, value: "a", children: [leaf] },
  cursor: {
    error: { startIndex: 1, lastIndex: 2, pattern: { name: "e" } },
    furthestError: { startIndex: 2, lastIndex: 3, pattern: { name: "f" } },
    tooDeep: { depth: 1025 },
  },
};
const v12Suggestion = { isComplete: true, errorAtIndex: 4, options: [{ text: "b", startIndex: 1 }], cursor: { tooDeep: {} } };
const v12Build = { patterns: { s: {} }, error: "Error: e", decorated: [["d", "s", "regex", "1"]] };

function fullResults() {
  return {
    build: buildResult("calc#0", v12Build, true),
    exec: execResult(v12Exec, "Error at line 1, column 2.", "ab"),
    autocomplete: autoCompleteResult(v12Suggestion),
  };
}

test("normalize reports every observable, and nothing else", () => {
  const results = fullResults();
  for (const [section, fields] of Object.entries(OBSERVED)) {
    const keys = Object.keys(results[section]).filter(key => key !== "build");
    assert.deepEqual(keys.sort(), [...fields].sort(), section);
    for (const field of fields) assert.notEqual(results[section][field], null, `${section}.${field}`);
  }
});

test("normalize follows each observable: changing it changes the result", () => {
  const changed = [
    ["exec", "matched", r => ({ ...r, ast: null })],
    ["exec", "ast", r => ({ ...r, ast: { ...r.ast, endIndex: 2 } })],
    ["exec", "error", r => ({ ...r, cursor: { ...r.cursor, error: { ...r.cursor.error, lastIndex: 9 } } })],
    ["exec", "furthest", r => ({ ...r, cursor: { ...r.cursor, furthestError: { ...r.cursor.furthestError, startIndex: 9 } } })],
    ["exec", "tooDeep", r => ({ ...r, cursor: { ...r.cursor, tooDeep: null } })],
  ];
  const base = execResult(v12Exec, "m", "ab");
  for (const [, field, change] of changed) {
    assert.notDeepEqual(execResult(change(v12Exec), "m", "ab")[field], base[field], field);
  }
  assert.notDeepEqual(execResult(v12Exec, "other", "ab").message, base.message);
  const suggestion = autoCompleteResult(v12Suggestion);
  for (const [field, change] of [
    ["complete", s => ({ ...s, isComplete: false })],
    ["at", s => ({ ...s, errorAtIndex: 5 })],
    ["options", s => ({ ...s, options: [] })],
    ["tooDeep", s => ({ ...s, cursor: null })],
  ]) {
    assert.notDeepEqual(autoCompleteResult(change(v12Suggestion))[field], suggestion[field], field);
  }
  assert.notDeepEqual(buildResult("k", { ...v12Build, decorated: [] }, true).decorated, buildResult("k", v12Build, true).decorated);
});

test("removing each compared field in turn is a difference of that kind", () => {
  const corpus = {
    files: [{ name: "calc", grammars: [{ key: "calc#0", cases: [{ id: 0, rule: "s", input: "ab", kind: "truncation" }] }] }],
  };
  const { build, exec, autocomplete } = fullResults();
  const want = indexResults([build, { id: 0, exec, autocomplete }]);
  assert.deepEqual(compare(corpus, want, want, "native").differences, []);

  // The build's error hides its other fields, so they are dropped from a
  // build that succeeded.
  const built = { ...build, error: null };
  const wantBuilt = indexResults([built, { id: 0, exec, autocomplete }]);
  for (const [section, fields] of Object.entries(OBSERVED)) {
    for (const field of fields) {
      const without = value => {
        const copy = { ...value };
        delete copy[field];
        return copy;
      };
      const lines =
        section === "build"
          ? [without(field === "error" ? build : built), { id: 0, exec, autocomplete }]
          : [built, { id: 0, exec, autocomplete, [section]: without({ exec, autocomplete }[section]) }];
      const expected = section === "build" && field === "error" ? want : wantBuilt;
      const { differences } = compare(corpus, expected, indexResults(lines), "native");
      assert.deepEqual(
        differences.map(d => d.kind),
        [`${section}.${field}`],
        `dropping ${section}.${field}`
      );
    }
  }
});

// ─── The allowlist's kinds, input patterns and named checks ─────────────

test("an entry can name several kinds, an input pattern and a check", () => {
  const ranOut = { kind: "exec.furthest", engine: "native", input: "x😀", expected: [0, 2, "s"], actual: [0, 3, "s"] };
  const entry = {
    id: "wide",
    kinds: ["exec.furthest", "autocomplete.at"],
    inputPattern: "\\p{Extended_Pictographic}$",
    check: "ranOutInsideAWideLastGrapheme",
    reason: "r",
    cites: "DESIGN.md §11",
  };
  assert.equal(applyAllowlist([entry], [ranOut]).unexplained.length, 0);
  // errorAtIndex is checked the same way.
  assert.equal(applyAllowlist([entry], [{ ...ranOut, kind: "autocomplete.at", expected: 2, actual: 3 }]).unexplained.length, 0);
  // Not a kind it names, not its input, not what the check allows.
  assert.equal(applyAllowlist([entry], [{ ...ranOut, kind: "exec.error" }]).unexplained.length, 1);
  assert.equal(applyAllowlist([entry], [{ ...ranOut, input: "x😀a" }]).unexplained.length, 1);
  assert.equal(applyAllowlist([entry], [{ ...ranOut, actual: [1, 3, "s"] }]).unexplained.length, 1);
  // A one-unit last grapheme gives the same number in both: no excuse.
  const narrow = { ...ranOut, input: "xé", expected: [0, 2, "s"], actual: [0, 2, "s"] };
  assert.equal(applyAllowlist([{ ...entry, inputPattern: undefined }], [{ ...narrow, actual: [0, 3, "s"] }]).unexplained.length, 1);
});

test("an entry with an unknown check or no kind is refused on load", () => {
  const dir = mkdtempSync(join(tmpdir(), "allowlist-"));
  const write = entries => {
    const path = join(dir, "allowlist.json");
    writeFileSync(path, JSON.stringify(entries));
    return path;
  };
  const base = { id: "x", kind: "exec.ast", reason: "r", cites: "DESIGN.md §1" };
  assert.equal(loadAllowlist(write([base])).length, 1);
  assert.throws(() => loadAllowlist(write([{ ...base, check: "nope" }])), /unknown check/);
  assert.throws(() => loadAllowlist(write([{ ...base, kind: undefined }])), /needs a `kind`/);
  assert.throws(() => loadAllowlist(write([{ ...base, cites: "my notes" }])), /must cite/);
});

// ─── Grammars built with options are replayed from their record ─────────

class FakePattern {
  constructor(type, name, value) {
    Object.assign(this, { type, name, value });
  }
}
const fakeApi = { Literal: class extends FakePattern { constructor(n, v) { super("literal", n, v); } }, Regex: class extends FakePattern { constructor(n, v) { super("regex", n, v); } } };

test("replayed options: params by value, recorded imports, recording decorators", async () => {
  const { grammarOptions, decorated } = replayOptions(fakeApi, {
    entry: "parse",
    originResource: "/root.cpat",
    params: [{ type: "literal", name: "v", value: "V" }, { type: "regex", name: "d", regex: "\\d" }],
    decorators: ["config"],
    resolveImport: [
      { resource: "a", origin: "/root.cpat", file: { resource: "a.cpat", expression: "x = \"X\"" } },
      { resource: "b", origin: null, error: "No Import" },
    ],
  });
  assert.equal(grammarOptions.originResource, "/root.cpat");
  assert.deepEqual(grammarOptions.params.map(p => [p.type, p.name, p.value]), [["literal", "v", "V"], ["regex", "d", "\\d"]]);
  assert.deepEqual(await grammarOptions.resolveImport("a", "/root.cpat"), { resource: "a.cpat", expression: "x = \"X\"" });
  await assert.rejects(grammarOptions.resolveImport("b", null), /^Error: No Import$/);
  await assert.rejects(grammarOptions.resolveImport("a", null), /unrecorded import "a" from null/);
  assert.equal(grammarOptions.resolveImportSync, undefined);

  grammarOptions.decorators.config({ name: "s", type: "regex" }, { b: 1, a: [2] });
  grammarOptions.decorators.config({ name: "t", type: "literal" });
  assert.deepEqual(decorated, [["config", "s", "regex", '{"b":1,"a":[2]}'], ["config", "t", "literal", null]]);
});

test("a build reports what its decorators received only when it has some", async () => {
  const api = { Grammar: { parseString: () => ({ a: 1 }), parse: async () => ({ b: 1 }) } };
  const plain = await replayBuild(api, { text: "a = 'a'" }, String);
  assert.deepEqual(buildResult("k", plain, false), { build: "k", error: null, rules: ["a"] });
  const asynchronous = await replayBuild(api, { text: "", options: { entry: "parse", decorators: ["d"] } }, String);
  assert.deepEqual(buildResult("k", asynchronous, true), { build: "k", error: null, rules: ["b"], decorated: [] });
});

test("grammars count as using imports, params and decorators by text or options", () => {
  assert.deepEqual(grammarUses({ text: 'import { a } from "x"\nuse params { b }\n@tokens([" "])\nc = a' }), {
    imports: true,
    params: true,
    decorators: true,
  });
  assert.deepEqual(grammarUses({ text: 'a = "@x" + "import {"' }), { imports: false, params: false, decorators: false });
  assert.deepEqual(grammarUses({ text: "a = b", options: { resolveImportSync: [], params: [{}], decorators: ["d"] } }), {
    imports: true,
    params: true,
    decorators: true,
  });
});

// ─── The generated non-ASCII corpus ──────────────────────────────────────

test("the committed unicode corpus is what the generator writes", () => {
  const committed = JSON.parse(readFileSync(join(corpusDir, "unicode.json"), "utf8"));
  assert.deepEqual(unicodeCorpus(loadCorpusFiles()), committed);
});

test("variants swap every space, insert non-ASCII words and end on wide characters", () => {
  const out = variants("a b");
  for (const space of SPACES) assert.ok(out.includes(`a${space}b`), JSON.stringify(space));
  assert.ok(out.includes("a 👍🏽b"));
  assert.ok(out.includes("a b😀"));
  assert.ok(out.includes("﻿a b"));
  // Without a space, words go in the middle, never inside a surrogate pair.
  assert.ok(variants("😀😀").includes("😀é😀"));
});
