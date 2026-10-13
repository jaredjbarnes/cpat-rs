// Parse results hold wasm memory until freed. The JS API frees each one as
// soon as it has read it, so a synchronous loop of parses doesn't grow wasm
// memory until it aborts (it did: the conformance harness ran out after
// tens of thousands of parses). Counted, never measured: `liveResults` is
// how many results wasm still holds.
import { test } from "node:test";
import assert from "node:assert/strict";
import { AutoComplete, Cursor, generateErrorMessage, patterns } from "./index.mjs";
import { liveResults } from "./wasm.mjs";

const { expr, num } = patterns`
  num = /\\d+/
  add = expr + "+" + expr
  expr = add | num
`;

test("exec, parse and generateErrorMessage leave no result in wasm", () => {
  const before = liveResults();
  for (let i = 0; i < 50; i++) {
    const { ast, cursor } = expr.exec("1+2+");
    generateErrorMessage(expr, cursor);
    assert.equal(ast, null);
    assert.equal(cursor.furthestError != null, true);
    assert.equal(expr.exec("1+2").ast.value, "1+2");
    assert.equal(num.parse(new Cursor("12")).value, "12");
  }
  assert.equal(liveResults(), before);
});

test("AutoComplete leaves no result in wasm", () => {
  const before = liveResults();
  const autoComplete = new AutoComplete(expr);
  for (let i = 0; i < 50; i++) {
    const suggestion = autoComplete.suggestFor("1+");
    assert.equal(suggestion.isComplete, false);
    assert.equal(suggestion.cursor.isRecording, true);
    assert.equal(suggestion.ast.value, "1");
  }
  assert.equal(liveResults(), before);
});
