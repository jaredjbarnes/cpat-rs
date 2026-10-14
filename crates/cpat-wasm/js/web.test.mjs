// js/web.mjs, the entry for browsers and bundlers, run in Node as a browser
// would: the wasm arrives as a fetch `Response`, and nothing it imports is a
// Node built-in (./web_imports.test.mjs checks that). Needs
// `wasm-pack build --target web --release --out-dir pkg-web`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import init, * as api from "./web.mjs";
import { parsesThroughTheApi } from "./testing/web_scenario.mjs";

const wasmUrl = new URL("../pkg-web/cpat_wasm_bg.wasm", import.meta.url);
const response = async () =>
  new Response(await readFile(wasmUrl), { headers: { "Content-Type": "application/wasm" } });

// One process, one sequence: the steps share the module's one wasm instance.
test("the web entry before init, after init, and next to the Node entry", async (t) => {
  await t.test("before init, using the API says wasm isn't loaded", () => {
    assert.throws(() => new api.Literal("a", "a"), /wasm isn't loaded/);
    assert.equal(api.grammar, undefined);
  });

  await t.test("after init from a fetch Response, a grammar parses as in v12", async () => {
    await init(response());
    parsesThroughTheApi(api);
  });

  await t.test("init again keeps the same instance", async () => {
    const before = api.grammar;
    await init(response());
    assert.equal(api.grammar, before);
    parsesThroughTheApi(api);
  });

  await t.test("the Node entry refuses to load a second wasm build", async () => {
    await assert.rejects(import("./index.mjs"), /different wasm build is already loaded/);
  });
});
