// v12's `AutoComplete`: what could come next in a text. The work is the
// core's (DESIGN §17); this shapes its answer as v12's `Suggestion`, in
// UTF-16 offsets, with each segment's pattern a place in (a copy of) the
// pattern AutoComplete was built with.

import { WasmAutoComplete } from "./wasm.mjs";
import { Cursor } from "./cursor.mjs";
import { ParseError } from "./parse_error.mjs";
import { takeSuggestionResult } from "./taken.mjs";
import { wrap } from "./pattern.mjs";
import { Tree } from "./tree.mjs";

const defaultOptions = { greedyPatternNames: [], customTokens: {} };

export class AutoComplete {
  constructor(pattern, options = defaultOptions) {
    this._pattern = pattern;
    this._options = options;
    // A copy at the pattern's place, which segment paths start from.
    this._root = pattern._root.cloneAt(pattern._path, undefined);
    this._wasm = new WasmAutoComplete(
      this._root,
      options.greedyPatternNames ?? [],
      JSON.stringify(options.customTokens ?? {}),
      Boolean(options.disableDedupe)
    );
  }

  suggestFor(text) {
    return this.suggestForWithCursor(new Cursor(text));
  }

  /** v12's: the suggestion for `cursor.text`, with `cursor` as its
   * cursor. As in v12 the cursor starts recording and takes the parse's
   * report (errors, matches, history), so it is changed. */
  suggestForWithCursor(cursor) {
    const text = cursor.text;
    const result = takeSuggestionResult(this._wasm.suggestFor(text));
    const suggestion = JSON.parse(result.json);
    if (suggestion.patternError != null) {
      throw new Error(suggestion.thrownMessage);
    }
    cursor.moveTo(0);
    cursor.startRecording();
    const report = result.cursor;
    if (report != null) {
      // Empty text isn't parsed on this cursor (v12 answers it with
      // `test("")` on a cursor of its own), so it has nothing to report.
      cursor._take({ cursor: report, tooDeep: suggestion.tooDeep }, this._root, []);
    }
    const tree = new Tree(text, result);
    const error = suggestion.error;

    return {
      isComplete: suggestion.isComplete,
      options: suggestion.options.map(option => ({
        text: option.text,
        startIndex: option.startIndex,
        suggestionSequence: option.suggestionSequence.map(segment => ({
          text: segment.text,
          pattern: wrap(this._root, segment.path),
        })),
      })),
      error: error == null ? null : new ParseError(error.start, error.last, this._pattern),
      errorAtIndex: suggestion.errorAtIndex,
      cursor,
      ast: tree.root == null ? null : tree.root.toNode(),
    };
  }

  static suggestFor(text, pattern, options) {
    return new AutoComplete(pattern, options).suggestFor(text);
  }

  static suggestForWithCursor(cursor, pattern, options) {
    return new AutoComplete(pattern, options).suggestForWithCursor(cursor);
  }
}
