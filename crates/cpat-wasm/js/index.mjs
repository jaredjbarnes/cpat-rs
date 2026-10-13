// clarity-pattern-parser's API (v12's src/index.ts) over cpat-rs's wasm
// build. Every runtime name v12 exports is exported here; README "The JS
// API" says what differs.

import { cpatGrammar } from "./wasm.mjs";
import { wrap } from "./pattern.mjs";

export { Node } from "./node.mjs";
export { compact, remove } from "./compact.mjs";
export { Grammar } from "./grammar.mjs";
export { AutoComplete } from "./auto_complete.mjs";
export { Cursor } from "./cursor.mjs";
export { ParseError } from "./parse_error.mjs";
export {
  Block,
  Context,
  Cut,
  Expression,
  Literal,
  Not,
  Optional,
  Reference,
  Regex,
  Repeat,
  RightAssociated,
  Sequence,
  TakeUntil,
  isTokenizablePattern,
} from "./patterns.mjs";
export { Query, $ } from "./query.mjs";
export { Selector } from "./selector.mjs";
export { patterns, createPatternsTemplate } from "./template.mjs";
export { generateErrorMessage } from "./error_message.mjs";
export { Tree } from "./tree.mjs";

/** The cpat grammar itself: the pattern that reads `.cpat` text. */
export const grammar = wrap(cpatGrammar());
