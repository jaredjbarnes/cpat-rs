// js/web.mjs's `initSync`, fed the .wasm bytes. Needs
// `wasm-pack build --target web --release --out-dir pkg-web`.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import * as api from "./web.mjs";
import { parsesThroughTheApi } from "./testing/web_scenario.mjs";

test("initSync with the .wasm bytes makes the API ready", () => {
  api.initSync(readFileSync(new URL("../pkg-web/cpat_wasm_bg.wasm", import.meta.url)));
  parsesThroughTheApi(api);
});

test("the web entry exports the API's names, plus init and initSync", async () => {
  const shared = await import("./api.mjs");
  assert.deepEqual(
    Object.keys(api).sort(),
    [...Object.keys(shared), "default", "initSync"].sort(),
  );
});
