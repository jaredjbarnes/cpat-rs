// The harness's own logic, on small made-up results: a shifted range is a
// difference, the allowlist explains only what it names, a stale entry is
// reported, and mutations are what the corpus says they are.
//
//   node --test conformance/test/*.test.mjs
import { test } from "node:test";
import assert from "node:assert/strict";
import { applyAllowlist } from "../lib/allowlist.mjs";
import { compare, indexResults } from "../lib/compare.mjs";
import { deletions, tokenBoundaries, truncations } from "../lib/mutate.mjs";

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
