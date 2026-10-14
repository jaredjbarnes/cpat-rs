// v12's exported `grammar` (the pattern that reads `.cpat` text). It is
// built by wasm, so it can't be a constant computed while modules load: an
// entry calls `loadGrammar` once its wasm build is installed, and the live
// binding then holds the pattern.

import { cpatGrammar } from "./wasm.mjs";
import { wrap } from "./pattern.mjs";
// `wrap` picks a class by type from what ./patterns.mjs registers.
import "./patterns.mjs";

/** The cpat grammar itself; undefined until the entry has loaded wasm. */
export let grammar;

export function loadGrammar() {
  grammar = wrap(cpatGrammar());
}
