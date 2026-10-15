// What a workload's results must agree on before it is timed, in a compact
// shape the native runner (bench/native/src/signature.rs) writes too. Every
// position is in UTF-16 units, as v12 reports them.

/** FNV-1a over a string's UTF-16 code units: cheap, and the same number in
 * Rust over `encode_utf16`. */
export function hash(text) {
  let h = 0x811c9dc5;
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i);
    h = Math.imul(h, 0x01000193) >>> 0;
  }
  return h;
}

/** Every node reachable from the root, iteratively (trees are deep). */
export function countNodes(root) {
  let count = 0;
  const stack = [root];
  while (stack.length > 0) {
    const node = stack.pop();
    count++;
    for (const child of node.children) stack.push(child);
  }
  return count;
}

const thrown = error => (error instanceof Error ? `${error.name}: ${error.message}` : String(error));

/** A grammar build: its rule names in order, or its error. */
export function buildSignature(build) {
  try {
    const names = Object.keys(build());
    return { rules: names.length, names: hash(names.join("\n")) };
  } catch (error) {
    return { threw: thrown(error) };
  }
}

/** One exec: a match is `[1, nodes, root length, root hash]`; a miss is
 * `[0, error start, error last]` (cpat's depth limit says so). */
export function execSignature(exec) {
  try {
    const { ast, cursor } = exec();
    if (cursor.tooDeep) return ["tooDeep"];
    if (ast == null) return [0, cursor.error?.startIndex ?? -1, cursor.error?.lastIndex ?? -1];
    return [1, countNodes(ast), ast.value.length, hash(ast.value)];
  } catch (error) {
    return [`threw ${thrown(error).slice(0, 120)}`];
  }
}

/** One suggestion: complete, where the text stops making sense, and the
 * options (text and start, in order). */
export function suggestSignature(suggest) {
  try {
    const s = suggest();
    const options = s.options.map(o => `${o.text}\u0000${o.startIndex}`).join("\u0001");
    return [s.isComplete ? 1 : 0, s.errorAtIndex ?? -1, s.options.length, hash(options)];
  } catch (error) {
    return [`threw ${thrown(error).slice(0, 120)}`];
  }
}
