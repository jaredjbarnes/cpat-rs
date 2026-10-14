// Node's wasm: pkg-node, built by
// `wasm-pack build --target nodejs --release --out-dir pkg-node`. It loads
// synchronously, so ./index.mjs imports this first and its API is ready
// when the import returns.
import { createRequire } from "node:module";
import { install } from "./wasm.mjs";
import { loadGrammar } from "./cpat_grammar.mjs";

if (install(createRequire(import.meta.url)("../pkg-node/cpat_wasm.js"))) loadGrammar();
