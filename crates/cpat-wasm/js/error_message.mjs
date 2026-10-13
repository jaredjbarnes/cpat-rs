// v12's `generateErrorMessage(pattern, cursor)`, worded by wasm from the
// cursor's furthest leaf match (DESIGN §16).

export function generateErrorMessage(pattern, cursor) {
  const leaf = cursor.leafMatch;
  const path = leaf.node == null ? undefined : cursor._leafPath();
  const end = leaf.node == null ? 0 : leaf.node.endIndex;
  return pattern._root.errorMessageAt(pattern._path, cursor.text, path, end);
}
