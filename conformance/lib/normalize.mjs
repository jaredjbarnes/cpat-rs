// The normalized shape every engine reports, so results compare as plain
// values. Positions are UTF-16 offsets, as v12's are.

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
