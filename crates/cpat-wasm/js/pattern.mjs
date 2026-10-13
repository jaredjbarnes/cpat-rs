// The base of every JS pattern class: v12's `Pattern` contract over a
// pattern living in wasm.
//
// A pattern object is a PLACE: a wasm root and the path to one instance in
// it (DESIGN §22). A pattern built in JS is its own root, at the empty
// path. Its `children` are places one step further, so a child knows what
// comes after it (`getNextTokens`) the way v12's child knows its parent,
// without any pattern in wasm holding a parent pointer (DESIGN §2).
// Composite constructors take COPIES of the patterns they are given, as
// v12's clone theirs.

import { Cursor, setPlaceWrapper } from "./cursor.mjs";
import { Tree } from "./tree.mjs";

const classesByType = new Map();

/** Which class stands for each v12 `type` (./patterns.mjs fills it). */
export function registerPatternClass(type, Class) {
  classesByType.set(type, Class);
}

/** The pattern at `path` in `root`, as an object of the class its type
 * names. */
export function wrap(root, path = []) {
  const Class = classesByType.get(root.typeAt(path)) ?? PatternBase;
  const pattern = Object.create(Class.prototype);
  pattern._root = root;
  pattern._path = path;
  return pattern;
}

setPlaceWrapper(wrap);

/** A wasm copy of `pattern`, for a composite to take over. */
export function owned(pattern) {
  if (!(pattern instanceof PatternBase)) {
    throw new TypeError(`expected a pattern, got ${pattern}`);
  }
  return pattern._root.cloneAt(pattern._path, undefined);
}

function decodePaths(flat, root) {
  const patterns = [];
  for (let at = 0; at < flat.length; ) {
    const length = flat[at++];
    patterns.push(wrap(root, Array.from(flat.subarray(at, at + length))));
    at += length;
  }
  return patterns;
}

/** v12 throws when a reference names nothing; so does this, with its
 * message. Anything else wrong with the pattern says what in cpat's words. */
function throwPatternError(result) {
  const missing = result.missingName;
  if (missing != null) {
    throw new Error(`Couldn't find '${missing}' pattern within tree.`);
  }
  throw new Error(result.patternError);
}

/** v12's `ParseResult`: `{ ast, cursor }`. `ast` becomes a `Node` the
 * first time it is read; `tree` is the read-only view it comes from. */
export function parseResult(text, result, root, prefix) {
  if (result.patternError != null) {
    throwPatternError(result);
  }
  const cursor = new Cursor(text);
  cursor._take(result, root, prefix);
  const tree = new Tree(text, result);
  let ast;
  return {
    get ast() {
      if (ast === undefined) {
        ast = tree.root == null ? null : tree.root.toNode();
      }
      return ast;
    },
    cursor,
    tree,
  };
}

export class PatternBase {
  constructor(root) {
    this._root = root;
    this._path = [];
  }

  get id() {
    return this._root.idAt(this._path);
  }

  get type() {
    return this._root.typeAt(this._path);
  }

  get name() {
    return this._root.nameAt(this._path);
  }

  /** The pattern this one is a child of, or `null` for a root. */
  get parent() {
    return this._path.length === 0 ? null : wrap(this._root, this._path.slice(0, -1));
  }

  get children() {
    if (this._children === undefined) {
      const count = this._root.childCountAt(this._path);
      this._children = Array.from({ length: count }, (_, i) => wrap(this._root, [...this._path, i]));
    }
    return this._children;
  }

  get startedOnIndex() {
    return 0;
  }

  exec(text, record = false) {
    const result =
      this._path.length === 0
        ? this._root.exec(text, record)
        : this._root.execAt(this._path, text, record);
    return parseResult(text, result, this._root, this._path);
  }

  test(text, record = false) {
    return this._path.length === 0
      ? this._root.test(text, record)
      : this._root.testAt(this._path, text, record);
  }

  /** v12 `parse(cursor)`: parses from `cursor.index` and reports to the
   * cursor; the node, or `null`. */
  parse(cursor) {
    const result = this._root.parseAt(this._path, cursor.text, cursor.index, cursor.isRecording);
    if (result.patternError != null) {
      throwPatternError(result);
    }
    cursor._take(result, this._root, this._path);
    const view = new Tree(cursor.text, result).root;
    if (view == null) {
      return null;
    }
    const node = view.toNode();
    cursor._index = node.lastIndex;
    return node;
  }

  clone(name) {
    return wrap(this._root.cloneAt(this._path, name));
  }

  getTokens() {
    return this._root.tokensAt(this._path);
  }

  getTokensAfter(childReference) {
    const index = this._childIndex(childReference);
    return index == null ? [] : this._root.tokensAfterAt(this._path, index);
  }

  getNextTokens() {
    return this._root.nextTokensAt(this._path);
  }

  getPatterns() {
    return decodePaths(this._root.patternsAt(this._path), this._root);
  }

  getPatternsAfter(childReference) {
    const index = this._childIndex(childReference);
    return index == null ? [] : decodePaths(this._root.patternsAfterAt(this._path, index), this._root);
  }

  getNextPatterns() {
    return decodePaths(this._root.nextPatternsAt(this._path), this._root);
  }

  find(predicate) {
    // A throw from the predicate stops the search and comes back out of
    // `findAt` as it was thrown.
    const found = this._root.findAt(this._path, {
      test: candidate => Boolean(predicate(wrap(candidate))),
    });
    return found == null ? null : wrap(this._root, Array.from(found));
  }

  isEqual(pattern) {
    return pattern instanceof PatternBase && this._root.isEqualAt(this._path, pattern._root, pattern._path);
  }

  // Which of this pattern's children `child` is: the same root, one step
  // below this place. Anything else is v12's "bogus child", which has
  // nothing after it.
  _childIndex(child) {
    if (!(child instanceof PatternBase) || child._root !== this._root) {
      return null;
    }
    const path = child._path;
    if (path.length !== this._path.length + 1 || this._path.some((step, i) => step !== path[i])) {
      return null;
    }
    return path[path.length - 1];
  }
}
