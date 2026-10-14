// The wasm module every other module calls, as live bindings an entry fills.
//
// Loading wasm differs by host: Node can require pkg-node synchronously, a
// browser or bundler has to fetch and instantiate pkg-web asynchronously. So
// no module here imports a build. An entry picks one and calls `install`
// (./index.mjs for Node, ./web.mjs for the web), and nothing calls wasm while
// a module is evaluated, so the API's modules load the same way under both.

let installed = null;

// Until an entry installs a build, every binding is a stand-in whose every
// use says what is missing, rather than "undefined is not a function".
function unloaded(name) {
  const fail = () => {
    throw new Error(
      `cpat: wasm isn't loaded (used ${name}). From js/web.mjs, await its default ` +
        "init() (or call initSync) before using the API; js/index.mjs loads it itself.",
    );
  };
  return new Proxy(function () {}, { get: fail, apply: fail, construct: fail });
}

export let WasmPattern = unloaded("Pattern");
export let WasmAutoComplete = unloaded("AutoComplete");
export let buildGrammar = unloaded("buildGrammar");
export let importsOf = unloaded("importsOf");
export let cpatGrammar = unloaded("cpatGrammar");
export let liveResults = unloaded("liveResults");
export let graphemeStarts = unloaded("graphemeStarts");

/** Point every binding at `build`, a wasm-pack build's exports. Installing
 * the same build again does nothing; a second, different build is refused:
 * patterns from two wasm instances can't be mixed (each owns its memory). */
export function install(build) {
  if (installed === build) return false;
  if (installed !== null) {
    throw new Error("cpat: a different wasm build is already loaded; use one entry (index.mjs or web.mjs) per program");
  }
  installed = build;
  WasmPattern = build.Pattern;
  WasmAutoComplete = build.AutoComplete;
  ({ buildGrammar, importsOf, cpatGrammar, liveResults, graphemeStarts } = build);
  return true;
}
