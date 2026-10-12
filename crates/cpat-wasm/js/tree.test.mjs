// Smoke test for the wasm build. Run after `wasm-pack build --target nodejs
// --out-dir pkg-node`:  node --test js/
import { test } from "node:test";
import assert from "node:assert/strict";
import { createRequire } from "node:module";
import { Tree } from "./tree.mjs";

const { Pattern } = createRequire(import.meta.url)("../pkg-node/cpat_wasm.js");

function fullName() {
  return Pattern.sequence("full-name", [
    Pattern.literal("first", "John"),
    Pattern.regex("space", "\\s+"),
    Pattern.literal("last", "Doe"),
  ]);
}

test("exec returns a tree the JS view can walk", () => {
  const text = "John Doe";
  const result = fullName().exec(text);
  const root = new Tree(text, result).root;

  assert.equal(result.matched, true);
  assert.equal(root.name, "full-name");
  assert.equal(root.type, "sequence");
  assert.equal(root.value, "John Doe");
  assert.deepEqual(
    root.children.map(c => [c.name, c.value, c.firstIndex, c.lastIndex]),
    [
      ["first", "John", 0, 3],
      ["space", " ", 4, 4],
      ["last", "Doe", 5, 7],
    ]
  );
  assert.equal(root.find(n => n.name === "last").parent.name, "full-name");
  assert.deepEqual(root.findAll(n => n.isLeaf).map(n => n.name), ["first", "space", "last"]);
});

test("positions are UTF-16 offsets, so values slice the JS string", () => {
  const pattern = Pattern.sequence("s", [
    Pattern.literal("dot", "🔴"),
    Pattern.regex("word", "[a-zé]+"),
  ]);
  const text = "🔴café";
  const root = new Tree(text, pattern.exec(text)).root;

  const word = root.children[1];
  assert.equal(word.startIndex, 2, "🔴 is two UTF-16 units");
  assert.equal(word.endIndex, 6);
  assert.equal(word.value, "café");
});

test("a failed parse reports the furthest error in UTF-16 offsets", () => {
  const result = fullName().exec("John Dot");

  assert.equal(result.matched, false);
  assert.deepEqual(Array.from(result.furthestError), [5, 7]);
  assert.equal(result.furthestErrorPattern, "last");
});

test("invalid patterns throw with a message that says what to fix", () => {
  assert.throws(() => Pattern.literal("empty", ""), /literal `empty` is empty/);
  assert.throws(() => Pattern.regex("anchored", "^a"), /remove the anchor/);
});

test("cut and choice work across the boundary", () => {
  const choice = Pattern.choice("choice", [
    Pattern.sequence("numeric", [
      Pattern.literal("open", "<<#"),
      Pattern.cut(),
      Pattern.regex("digits", "[0-9]+"),
      Pattern.literal("close", ">>"),
    ]),
    Pattern.sequence("textual", [
      Pattern.literal("open", "<<"),
      Pattern.regex("anything", "[^>]+"),
      Pattern.literal("close", ">>"),
    ]),
  ]);

  assert.equal(choice.test("<<#42>>"), true);
  assert.equal(choice.test("<<abc>>"), true);
  assert.equal(choice.test("<<#abc>>"), false, "committed after <<#");
});

test("an unbounded repeat comes back as an infinite-repeat node", () => {
  const text = "3,3,7,";
  const numbers = Pattern.sequence("list", [
    Pattern.repeat("numbers", Pattern.regex("digit", "\\d"), {
      divider: Pattern.literal("comma", ","),
      trimDivider: true,
    }),
    Pattern.literal("end", ","),
  ]);
  const root = new Tree(text, numbers.exec(text)).root;
  const repeat = root.children[0];

  assert.equal(repeat.type, "infinite-repeat");
  assert.equal(repeat.name, "numbers");
  assert.equal(repeat.value, "3,3,7", "the trailing divider is trimmed off");
  assert.deepEqual(
    repeat.children.map(c => [c.type, c.name, c.firstIndex, c.lastIndex]),
    [
      ["regex", "digit", 0, 0],
      ["literal", "comma", 1, 1],
      ["regex", "digit", 2, 2],
      ["literal", "comma", 3, 3],
      ["regex", "digit", 4, 4],
    ]
  );
});

test("a bounded repeat comes back as a finite-repeat node", () => {
  const digits = Pattern.repeat("digits", Pattern.regex("digit", "\\d"), { min: 2, max: 3 });

  const root = new Tree("123", digits.exec("123")).root;
  assert.equal(root.type, "finite-repeat");
  assert.equal(root.children.length, 3);

  const tooShort = digits.exec("1");
  assert.equal(tooShort.matched, false);
  assert.equal(tooShort.furthestErrorPattern, "digits", "a min failure is the repeat's own");

  assert.equal(digits.test("1234"), false, "max stops it short of the text");
});

test("repeat options default and validate", () => {
  const plus = Pattern.repeat("plus", Pattern.literal("a", "a"));
  assert.equal(plus.test("aaa"), true);

  const unbounded = Pattern.repeat("all", Pattern.literal("a", "a"), { max: Infinity });
  assert.equal(new Tree("aaaa", unbounded.exec("aaaa")).root.type, "infinite-repeat");

  assert.throws(
    () => Pattern.repeat("bad", Pattern.literal("a", "a"), { min: -1 }),
    /min must be a whole number/
  );
  assert.throws(
    () => Pattern.repeat("bad", Pattern.literal("a", "a"), { divider: "," }),
    /divider must be a Pattern/
  );
});

test("a bound too large to double still repeats", () => {
  // wasm's usize is 32 bits: doubling 2^31 for the divider slots used to
  // wrap to 0, and the repeat then failed every input on min.
  for (const max of [2 ** 31, 2 ** 32 - 1]) {
    for (const trimDivider of [false, true]) {
      const repeat = Pattern.repeat("r", Pattern.literal("a", "a"), {
        max,
        divider: Pattern.literal("c", ","),
        trimDivider,
      });
      assert.equal(repeat.test("a,a"), true, `max ${max}, trimDivider ${trimDivider}`);
    }
  }
});

test("a recursive grammar: references resolve, each level its own instance", () => {
  // expr = "(" expr ")" | "x"
  const expr = Pattern.choice("expr", [
    Pattern.sequence("group", [
      Pattern.literal("open", "("),
      Pattern.reference("expr"),
      Pattern.literal("close", ")"),
    ]),
    Pattern.literal("x", "x"),
  ]);
  const text = "((x))";
  const result = expr.exec(text);
  const root = new Tree(text, result).root;

  assert.equal(result.matched, true);
  assert.equal(result.patternError, undefined);
  const inner = root.children[1];
  assert.deepEqual(
    [root.name, inner.name, inner.children.map(c => c.value)],
    ["group", "group", ["(", "x", ")"]]
  );
  assert.equal(expr.test("((x)"), false);
});

test("a context puts definitions in scope; a reference can rename its target", () => {
  const names = Pattern.sequence("names", [
    Pattern.literal("john", "John"),
    Pattern.reference("gap", "space"),
    Pattern.literal("jane", "Jane"),
  ]);
  const context = Pattern.context("context", names, [Pattern.literal("space", " ")]);
  const text = "John Jane";
  const root = new Tree(text, context.exec(text)).root;

  assert.deepEqual(root.children.map(c => c.name), ["john", "gap", "jane"]);
  assert.equal(context.test("John Bob"), false);
});

test("a reference that names nothing is reported, not thrown", () => {
  const pattern = Pattern.sequence("s", [Pattern.reference("missing")]);

  const result = pattern.exec("a");

  assert.equal(result.matched, false);
  assert.match(result.patternError, /`missing`/);
  assert.throws(() => pattern.check(), /`missing`/);
});
