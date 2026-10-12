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
