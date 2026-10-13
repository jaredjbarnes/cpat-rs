// v12's `Cursor`: the text, a position that steps a character (grapheme)
// at a time, where a direct `parse` starts, and what a parse reported (the
// current and furthest errors, the leaf matches and, while recording, every
// error and match in order). Positions are UTF-16 offsets; error
// `lastIndex`es are inclusive.
//
// The parse itself runs in wasm. Its report crosses as JSON and is read the
// first time a property asks for it. Patterns in the report are named by
// their path from the pattern that parsed (DESIGN §22), and become pattern
// objects only when read. What v12's patterns call on the cursor while they
// parse (`recordMatch`, `recordErrorAt`, the choice scopes) works on that
// report as v12's `CursorHistory` does, but no wasm parse calls it: the
// next parse's report replaces what was recorded by hand.

import { Node } from "./node.mjs";
import { ParseError } from "./parse_error.mjs";
import { graphemeStarts } from "./wasm.mjs";

let wrapPlace = null;

/** Set by ./pattern.mjs, which can't be imported here without a cycle. */
export function setPlaceWrapper(wrap) {
  wrapPlace = wrap;
}

const NOTHING = Object.freeze({ pattern: null, node: null });

function emptyHistory() {
  return { error: null, furthestError: null, errors: [], records: [], leafMatches: [], rootMatch: null, matches: [] };
}

// v12's `_charMap` and `_charSize`: for every UTF-16 unit, where its
// character starts, and for every unit, how long its character is. The
// boundaries are the parser's (wasm `graphemeStarts`, DESIGN §22).
function characters(text) {
  const map = [];
  const size = [];
  const starts = graphemeStarts(text);
  for (let i = 0; i < starts.length; i++) {
    const start = starts[i];
    const length = (i + 1 < starts.length ? starts[i + 1] : text.length) - start;
    for (let unit = 0; unit < length; unit++) {
      map.push(start);
      size.push(length);
    }
  }
  return { map, size };
}

// Two places are the same pattern instance when they name the same path in
// the same root; any other object only when it is the same object.
function samePattern(a, b) {
  if (a === b) {
    return true;
  }
  if (a?._root == null || b?._root == null || a._root !== b._root) {
    return false;
  }
  return a._path.length === b._path.length && a._path.every((step, i) => step === b._path[i]);
}

export class Cursor {
  constructor(text) {
    this._text = text;
    this._index = 0;
    // `null` once a parse has reported: its report says whether it recorded.
    this._recording = false;
    this._result = null;
    this._parsed = null;
    this._chars = null;
    this._choiceScopes = [];
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
    return this._recording ?? this._raw().isRecording;
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

  /** The character at the cursor: a whole grapheme ("🔴", not half of it). */
  get currentChar() {
    const index = this.getCharStartIndex(this._index);
    return this._text.slice(index, index + this._characters().size[index]);
  }

  hasNext() {
    const { map, size } = this._characters();
    const index = map[this._index];
    return index + size[index] < this.length;
  }

  next() {
    if (this.hasNext()) {
      const { map, size } = this._characters();
      const index = map[this._index];
      this.moveTo(index + size[index]);
    }
  }

  hasPrevious() {
    const { map } = this._characters();
    const index = map[this._index];
    return (map[index - 1] ?? -1) >= 0;
  }

  previous() {
    if (this.hasPrevious()) {
      const { map } = this._characters();
      const index = map[this._index];
      this.moveTo(map[index - 1] ?? -1);
    }
  }

  /** Moves to the start of the character holding `position`; a position
   * outside the text is ignored. */
  moveTo(position) {
    if (position >= 0 && position < this.length) {
      this._index = this._characters().map[position];
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

  // v12's three, as written: outside the text they answer what v12's do
  // (`undefined`, `NaN`).
  getCharStartIndex(index) {
    return this._characters().map[index];
  }

  getCharEndIndex(index) {
    const startIndex = this.getCharStartIndex(index);
    return startIndex + this._characters().size[startIndex];
  }

  getCharLastIndex(index) {
    return this.getCharEndIndex(index) - 1;
  }

  startRecording() {
    this._recording = true;
  }

  stopRecording() {
    this._recording = false;
  }

  // ── v12's CursorHistory, by hand ───────────────────────────────────────

  recordMatch(pattern, node) {
    const history = this._history();
    if (this.isRecording) {
      history.matches.push({ pattern, node });
      history.records.push({ pattern, ast: node, error: null });
    }
    history.rootMatch = { pattern, node };

    const leaf = this.leafMatch.node;
    if (leaf == null || node.lastIndex > leaf.lastIndex) {
      history.leafMatches = [{ pattern, node }];
    } else if (node.lastIndex === leaf.lastIndex) {
      // v12 keeps a tie unless it is a sibling of, or above, a match
      // already kept: one of their ancestors is its parent.
      const parent = pattern.parent;
      const isAncestor = history.leafMatches.some(m => {
        for (let at = m.pattern?.parent; at != null; at = at.parent) {
          if (parent != null && samePattern(at, parent)) {
            return true;
          }
        }
        return false;
      });
      if (!isAncestor) {
        history.leafMatches.unshift({ pattern, node });
      }
    }
  }

  recordErrorAt(startIndex, lastIndex, onPattern) {
    const history = this._history();
    const error = new ParseError(startIndex, lastIndex, onPattern);
    history.error = error;
    if (history.furthestError == null || lastIndex > history.furthestError.lastIndex) {
      history.furthestError = error;
    }
    if (this.isRecording) {
      history.errors.push(error);
      history.records.push({ pattern: onPattern, ast: null, error });
    }
  }

  /** exec's "the whole input wasn't matched": the current error, but the
   * furthest only when nothing else failed. */
  recordSummaryErrorAt(startIndex, lastIndex, onPattern) {
    const history = this._history();
    const furthest = history.furthestError;
    this.recordErrorAt(startIndex, lastIndex, onPattern);
    if (furthest != null) {
      history.furthestError = furthest;
    }
  }

  resolveError() {
    this._history().error = null;
  }

  // ── Committed choice (cut), v12's scopes ───────────────────────────────
  // A choice pushes a scope while it tries its alternatives; a cut marks
  // the innermost one. In cpat the parse keeps these flags itself (DESIGN
  // §4); here they are v12's bookkeeping for code that drives a cursor.

  pushChoiceScope() {
    this._choiceScopes.push({ cut: false });
  }

  popChoiceScope() {
    this._choiceScopes.pop();
  }

  markCut() {
    this.setCut(true);
  }

  isCutInCurrentScope() {
    return this._choiceScopes.at(-1)?.cut === true;
  }

  clearCut() {
    this.setCut(false);
  }

  getCut() {
    return this.isCutInCurrentScope();
  }

  setCut(value) {
    const top = this._choiceScopes.at(-1);
    if (top != null) {
      top.cut = value;
    }
  }

  /** Takes a parse's report: `result` is wasm's `ExecResult`, `root` and
   * `prefix` the place of the pattern that parsed. */
  _take(result, root, prefix) {
    this._result = result;
    this._recording = null;
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

  _characters() {
    return this._chars ?? (this._chars = characters(this._text));
  }

  /** The report, to be changed by the `record*` methods. */
  _history() {
    return this._read();
  }

  _read() {
    if (this._parsed != null) {
      return this._parsed;
    }
    if (this._result == null) {
      return (this._parsed = emptyHistory());
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
