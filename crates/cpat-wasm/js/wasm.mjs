// The wasm module, built for Node by
// `wasm-pack build --target nodejs --release --out-dir pkg-node`.
// Every other module imports it from here, so a build for another target
// changes this one line.
import { createRequire } from "node:module";

const wasm = createRequire(import.meta.url)("../pkg-node/cpat_wasm.js");

export const WasmPattern = wasm.Pattern;
export const WasmAutoComplete = wasm.AutoComplete;
export const { buildGrammar, importsOf, cpatGrammar, liveResults } = wasm;
