// v12's `Cursor`, as far as JS can see one: the text, where a direct
// `parse` starts, and what a parse reported (the current and furthest
// errors, the leaf matches and, while recording, every error and match in
// order). Positions are UTF-16 offsets; error `lastIndex`es are inclusive.
//
// The parse itself runs in wasm. Its report crosses as JSON and is read the
// first time a property asks for it. Patterns in the report are named by
// their path from the pattern that parsed (DESIGN §22), and become pattern
// objects only when read.

import { Node } from "./node.mjs";
import { ParseError } from "./parse_error.mjs";

let wrapPlace = null;

/** Set by ./pattern.mjs, which can't be imported here without a cycle. */
export function setPlaceWrapper(wrap) {
  wrapPlace = wrap;
}

const NOTHING = Object.freeze({ pattern: null, node: null });

export class Cursor {
  constructor(text) {
    this._text = text;
    this._index = 0;
    this._recording = false;
    this._result = null;
  }

  get text() {
    return this._text;
  }

  get length() {
    return this._text.length;
  }

  /** Where the next direct `parse` starts; after one that matched, the
   * last index of what it matched (v12 leaves the cursor on it). */
  get index() {
    return this._index;
  }

  get isOnFirst() {
    return this._index === 0;
  }

  get isOnLast() {
    return this._index === this.getLastIndex();
  }

  get isRecording() {
    return this._result == null ? this._recording : this._raw().isRecording;
  }

  get hasError() {
    return this.error != null;
  }

  get error() {
    return this._read().error;
  }

  get furthestError() {
    return this._read().furthestError;
  }

  get errors() {
    return this._read().errors;
  }

  get records() {
    return this._read().records;
  }

  get leafMatches() {
    return this._read().leafMatches;
  }

  get leafMatch() {
    const leaves = this.leafMatches;
    return leaves.length === 0 ? NOTHING : leaves[leaves.length - 1];
  }

  get rootMatch() {
    return this._read().rootMatch ?? NOTHING;
  }

  get allMatchedNodes() {
    return this._read().matches.map(m => m.node);
  }

  get allMatchedPatterns() {
    return this._read().matches.map(m => m.pattern);
  }

  /** `[maxDepth, index]` when the input nested deeper than the parser
   * allows and the parse stopped there (v12 throws a RangeError). */
  get tooDeep() {
    return this._result?.tooDeep ?? null;
  }

  moveTo(position) {
    if (position >= 0 && position < this.length) {
      this._index = position;
    }
  }

  moveToFirstChar() {
    this._index = 0;
  }

  moveToLastChar() {
    this._index = this.getLastIndex();
  }

  getLastIndex() {
    return this.length - 1;
  }

  substring(first, last) {
    return this._text.slice(first, last + 1);
  }

  startRecording() {
    this._recording = true;
  }

  stopRecording() {
    this._recording = false;
  }

  /** Takes a parse's report: `result` is wasm's `ExecResult`, `root` and
   * `prefix` the place of the pattern that parsed. */
  _take(result, root, prefix) {
    this._result = result;
    this._json = result.cursor;
    this._root = root;
    this._prefix = prefix;
    this._parsedJson = null;
    this._parsed = null;
  }

  /** The leaf match's path from the pattern that parsed, for
   * `generateErrorMessage`. */
  _leafPath() {
    const raw = this._raw().leafMatches;
    return raw.length === 0 ? undefined : raw[raw.length - 1].path;
  }

  _raw() {
    return this._parsedJson ?? (this._parsedJson = JSON.parse(this._json));
  }

  _read() {
    if (this._result == null) {
      return { error: null, furthestError: null, errors: [], records: [], leafMatches: [], rootMatch: null, matches: [] };
    }
    if (this._parsed != null) {
      return this._parsed;
    }

    const raw = this._raw();
    const error = e => (e == null ? null : new ParseError(e.start, e.last, this._pattern(e)));
    const found = m => {
      if (m == null) {
        return null;
      }
      const pattern = this._pattern(m);
      const node = new Node(pattern.type, m.name, m.start, m.end - 1, [], this._text.slice(m.start, m.end));
      return { pattern, node };
    };
    const matches = [];
    const records = raw.records.map(record => {
      if (record.match != null) {
        const m = found(record.match);
        matches.push(m);
        return { pattern: m.pattern, error: null, ast: m.node };
      }
      const e = error(record.error);
      return { pattern: e.pattern, error: e, ast: null };
    });

    this._parsed = {
      error: error(raw.error),
      furthestError: error(raw.furthestError),
      errors: raw.errors.map(error),
      records,
      leafMatches: raw.leafMatches.map(found),
      rootMatch: found(raw.rootMatch),
      matches,
    };
    return this._parsed;
  }

  // A recorded error or a match names its pattern by path. An ordinary
  // parse keeps no path for its errors (DESIGN §15), so those name the
  // pattern by what it is called and its id only.
  _pattern(record) {
    if (record.path == null) {
      return Object.freeze({ id: record.id, name: record.name, type: undefined });
    }
    return wrapPlace(this._root, this._prefix.concat(record.path));
  }
}
