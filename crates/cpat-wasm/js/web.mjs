// clarity-pattern-parser's API for browsers and bundlers, over pkg-web
// (`wasm-pack build --target web --release --out-dir pkg-web`). Nothing here
// or below uses a Node built-in. Wasm has to be fetched and instantiated
// before the API works, which can't happen while modules load, so the API
// waits for `init`:
//
//   import init, { patterns } from "cpat-wasm/js/web.mjs";
//   await init(); // fetches pkg-web/cpat_wasm_bg.wasm next to the glue
//
// `init(source)` also takes what wasm-bindgen's init takes: a URL, a fetch
// `Response` (or a promise of one), the .wasm bytes, or a
// `WebAssembly.Module`. `initSync(bytesOrModule)` instantiates synchronously.

import initWasm, * as build from "../pkg-web/cpat_wasm.js";
import { install } from "./wasm.mjs";
import { loadGrammar } from "./cpat_grammar.mjs";

export * from "./api.mjs";

/** Fetch (or take) the wasm and make the API ready. Safe to call again. */
export default async function init(source) {
  await initWasm(source === undefined ? undefined : { module_or_path: source });
  ready();
}

/** `init` for bytes or a compiled module already in hand. */
export function initSync(source) {
  build.initSync({ module: source });
  ready();
}

function ready() {
  if (install(build)) loadGrammar();
}
