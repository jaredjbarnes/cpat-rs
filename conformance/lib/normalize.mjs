// The normalized shape every engine reports, so results compare as plain
// values. Positions are UTF-16 offsets, as v12's are. The native engine
// writes the same keys (conformance/native/src/normalize.rs, engine.rs);
// test/harness.test.mjs fails if a field is dropped here or in compare.mjs.

/** The tree, pre-order, one row per node: `depth|type|name|start|end`.
 * A node whose value isn't the text it spans gets a `value!` row after it:
 * values are compared without storing every node's text. */
export function astRows(ast, input) {
  if (ast == null) return null;
  const rows = [];
  const stack = [[ast, 0]];
  while (stack.length > 0) {
    const [node, depth] = stack.pop();
    rows.push(`${depth}|${node.type}|${node.name}|${node.startIndex}|${node.endIndex}`);
    if (node.value !== input.slice(node.startIndex, node.endIndex)) rows.push(`value!${JSON.stringify(node.value)}`);
    for (let i = node.children.length - 1; i >= 0; i--) stack.push([node.children[i], depth + 1]);
  }
  return rows.join("\n");
}

/** `[start, last, pattern name]`, `last` inclusive as v12's `lastIndex`. */
export function errorTriple(error) {
  return error == null ? null : [error.startIndex, error.lastIndex, error.pattern?.name ?? null];
}

/** What an engine threw, by class and message. */
export function threw(error) {
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error);
}

/** A build: what it threw, its rule names in order, and, when the grammar
 * has custom decorators, what each received (replay.mjs `decoration`). */
export function buildResult(key, { patterns, error, decorated }, hasDecorators) {
  return {
    build: key,
    error,
    rules: patterns == null ? null : Object.keys(patterns),
    ...(hasDecorators ? { decorated } : {}),
  };
}

/** An exec: `{ ast, cursor }` as v12's `exec` returns it, and the error
 * message generated for it (or `{ threw }`). */
export function execResult({ ast, cursor }, message, input) {
  return {
    matched: ast != null,
    ast: astRows(ast, input),
    error: errorTriple(cursor.error),
    furthest: errorTriple(cursor.furthestError),
    message,
    // cpat's depth limit (README, DESIGN §11); v12 has none.
    ...(cursor.tooDeep ? { tooDeep: true } : {}),
  };
}

/** A suggestion, as v12's `suggestFor` returns it. */
export function autoCompleteResult(suggestion) {
  return {
    complete: suggestion.isComplete,
    at: suggestion.errorAtIndex,
    options: suggestion.options.map(o => [o.text, o.startIndex]),
    ...(suggestion.cursor?.tooDeep ? { tooDeep: true } : {}),
  };
}
