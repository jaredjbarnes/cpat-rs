// js/index.mjs exports every runtime name clarity-pattern-parser v12's
// src/index.ts exports. A name missing here fails the import itself.
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  isTokenizablePattern,
  Block,
  Node,
  Grammar,
  AutoComplete,
  Sequence,
  Cursor,
  Context,
  Expression,
  Cut,
  Literal,
  Not,
  Optional,
  ParseError,
  Reference,
  RightAssociated,
  Regex,
  Repeat,
  Query,
  Selector,
  grammar,
  patterns,
  compact,
  remove,
  generateErrorMessage,
  // Not in v12's index.ts, but part of the API the milestone asks for:
  // `TakeUntil` (v12 exports it from its own module), `$` (from
  // query.ts) and `createPatternsTemplate` (from patterns.ts).
  TakeUntil,
  $,
  createPatternsTemplate,
} from "./index.mjs";

// The rest of v12's index.ts, and why each isn't imported:
const SKIPPED = {
  BasePattern: "TS-internal: the abstract base the built-ins share; a JS pattern is any object of the pattern classes",
  CursorHistory: "TS-internal: the cursor's bookkeeping; its reports are on Cursor",
  Match: "TS-internal, and type-only: an interface ({ pattern, node })",
  HistoryRecord: "TS-internal, and type-only: an interface ({ pattern, error, ast })",
  TokenizablePattern: "type-only: an interface",
  AutoCompleteOptions: "type-only: an interface",
  Suggestion: "type-only: an interface",
  SuggestionOption: "type-only: an interface",
  ParseResult: "type-only: an interface ({ ast, cursor })",
  Pattern: "type-only: an interface",
};

const IMPORTED = {
  isTokenizablePattern,
  Block,
  Node,
  Grammar,
  AutoComplete,
  Sequence,
  Cursor,
  Context,
  Expression,
  Cut,
  Literal,
  Not,
  Optional,
  ParseError,
  Reference,
  RightAssociated,
  Regex,
  Repeat,
  Query,
  Selector,
  grammar,
  patterns,
  compact,
  remove,
  generateErrorMessage,
  TakeUntil,
  $,
  createPatternsTemplate,
};

// v12's index.ts export list, in its order.
const V12_EXPORTS = [
  "BasePattern", "TokenizablePattern", "isTokenizablePattern", "Block", "Node", "Grammar",
  "AutoComplete", "AutoCompleteOptions", "Suggestion", "SuggestionOption", "Sequence", "Cursor",
  "CursorHistory", "Match", "Context", "Expression", "Cut", "Literal", "Not", "Optional",
  "ParseError", "ParseResult", "Pattern", "Reference", "RightAssociated", "Regex", "Repeat",
  "Query", "Selector", "grammar", "patterns", "compact", "remove", "generateErrorMessage",
  "HistoryRecord",
];

test("every v12 export is either imported and defined, or skipped with a reason", () => {
  for (const name of V12_EXPORTS) {
    const imported = Object.hasOwn(IMPORTED, name);
    const skipped = Object.hasOwn(SKIPPED, name);
    assert.ok(imported !== skipped, `${name}: imported ${imported}, skipped ${skipped}`);
    if (imported) {
      assert.notEqual(IMPORTED[name], undefined, `${name} is undefined`);
    }
  }
  for (const [name, value] of Object.entries(IMPORTED)) {
    assert.notEqual(value, undefined, `${name} is undefined`);
  }
});

test("the exported grammar reads cpat text", () => {
  const { ast } = grammar.exec(`name = "John"`);
  assert.equal(ast.value, `name = "John"`);
  assert.ok(ast.find(n => n.name === "patternName"));
});
