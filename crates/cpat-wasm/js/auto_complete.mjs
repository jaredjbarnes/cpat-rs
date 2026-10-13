// v12's `AutoComplete`: what could come next in a text. The work is the
// core's (DESIGN §17); this shapes its answer as v12's `Suggestion`, in
// UTF-16 offsets, with each segment's pattern a place in (a copy of) the
// pattern AutoComplete was built with.

import { WasmAutoComplete } from "./wasm.mjs";
import { Cursor } from "./cursor.mjs";
import { ParseError } from "./parse_error.mjs";
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
    const result = this._wasm.suggestFor(text);
    const suggestion = JSON.parse(result.json);
    if (suggestion.patternError != null) {
      throw new Error(suggestion.patternError);
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
      cursor: new Cursor(text),
      ast: tree.root == null ? null : tree.root.toNode(),
    };
  }

  suggestForWithCursor(cursor) {
    return this.suggestFor(cursor.text);
  }
}
