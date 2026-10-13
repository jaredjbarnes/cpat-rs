// Parse results come out of wasm as objects that hold their tree and
// report in wasm memory until `free()`. wasm-bindgen would free them from a
// FinalizationRegistry, which runs only when the event loop turns, so a
// synchronous loop of parses (an ordinary way to use v12) grew wasm memory
// until it ran out and the module aborted. The conformance harness found
// it. Each result is read once into plain JS and freed at once.

/** An `ExecResult`'s fields, with the wasm object freed. */
export function takeExecResult(result) {
  if (result == null) {
    return result;
  }
  const taken = {
    patternError: result.patternError,
    thrownMessage: result.thrownMessage,
    tooDeep: result.tooDeep,
    cursor: result.cursor,
    nodes: result.nodes,
    children: result.children,
    names: result.names,
  };
  result.free();
  return taken;
}

/** A `SuggestionResult`'s fields, with the wasm object freed. */
export function takeSuggestionResult(result) {
  const taken = {
    json: result.json,
    cursor: result.cursor,
    nodes: result.nodes,
    children: result.children,
    names: result.names,
  };
  result.free();
  return taken;
}
