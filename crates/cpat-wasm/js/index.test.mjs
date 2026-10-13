// js/index.mjs exports every runtime name clarity-pattern-parser v12's
// src/index.ts exports. A name missing here fails the import itself.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
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

// ─── Every public member of v12's Cursor and pattern classes ───
//
// Listed from v12 by walking each class's prototype chain for names not
// starting with `_` (TS-private members are `_`-prefixed there). Each is
// either on the JS object here or a deviation below, which DESIGN §22 must
// name. A member that disappears, or a deviation that stops being one,
// fails the test.

const PATTERN_MEMBERS = [
  "children", "clone", "exec", "find", "getNextPatterns", "getNextTokens", "getPatterns",
  "getPatternsAfter", "getTokens", "getTokensAfter", "id", "isEqual", "name", "parent", "parse",
  "startedOnIndex", "test", "type",
];

const V12_MEMBERS = {
  Cursor: [
    "allMatchedNodes", "allMatchedPatterns", "clearCut", "currentChar", "error", "errors",
    "furthestError", "getCharEndIndex", "getCharLastIndex", "getCharStartIndex", "getCut",
    "getLastIndex", "hasError", "hasNext", "hasPrevious", "index", "isCutInCurrentScope",
    "isOnFirst", "isOnLast", "isRecording", "leafMatch", "leafMatches", "length", "markCut",
    "moveTo", "moveToFirstChar", "moveToLastChar", "next", "popChoiceScope", "previous",
    "pushChoiceScope", "recordErrorAt", "recordMatch", "recordSummaryErrorAt", "records",
    "resolveError", "rootMatch", "setCut", "startRecording", "stopRecording", "substring", "text",
  ],
  Literal: [...PATTERN_MEMBERS, "token"],
  Regex: [...PATTERN_MEMBERS, "regex", "setTokens"],
  Sequence: [...PATTERN_MEMBERS, "childBlockedAtEndOfText"],
  Expression: [
    ...PATTERN_MEMBERS, "atomPatterns", "binaryPatterns", "build", "infixPatterns",
    "originalPatterns", "postfixPatterns", "prefixPatterns",
  ],
  Optional: PATTERN_MEMBERS,
  Not: PATTERN_MEMBERS,
  Cut: PATTERN_MEMBERS,
  Repeat: [...PATTERN_MEMBERS, "max", "min", "options", "pattern"],
  Reference: [...PATTERN_MEMBERS, "getReferencePatternSafely"],
  Context: [...PATTERN_MEMBERS, "getPatternWithinContext", "getPatternsWithinContext"],
  RightAssociated: PATTERN_MEMBERS,
  Block: PATTERN_MEMBERS,
  TakeUntil: [...PATTERN_MEMBERS, "setTokens"],
};

// What v12 has that JS doesn't, and why (each named in DESIGN §22).
const MEMBER_DEVIATIONS = {
  "Sequence.childBlockedAtEndOfText":
    "v12's Expression asks it mid-parse with a live cursor; the parse runs in wasm, where the choice asks the sequence itself",
};

// v12's one public setter, `parent =` on every pattern: a place's parent is
// its path (DESIGN §22), so it has none.
const SETTER_DEVIATION = "`parent` setter";

const lit = () => new Literal("a", "a");
const INSTANCES = {
  Cursor: () => new Cursor("x"),
  Literal: lit,
  Regex: () => new Regex("r", "a"),
  Sequence: () => new Sequence("s", [lit()]),
  Expression: () => new Expression("e", [lit()]),
  Optional: () => new Optional("o", lit()),
  Not: () => new Not("n", lit()),
  Cut: () => new Cut(),
  Repeat: () => new Repeat("r", lit()),
  Reference: () => new Reference("a"),
  Context: () => new Context("c", lit(), []),
  RightAssociated: () => new RightAssociated(lit()),
  Block: () => new Block("b", new Literal("o", "{"), null, new Literal("c", "}")),
  TakeUntil: () => new TakeUntil("t", lit()),
};

function designSection22() {
  const design = readFileSync(new URL("../../../docs/DESIGN.md", import.meta.url), "utf8");
  const start = design.indexOf("## 22. ");
  const end = design.indexOf("## 23. ", start);
  return design.slice(start, end);
}

test("every v12 Cursor and pattern member is in JS, or a deviation DESIGN §22 names", () => {
  for (const [className, members] of Object.entries(V12_MEMBERS)) {
    const instance = INSTANCES[className]();
    for (const member of members) {
      const key = `${className}.${member}`;
      const deviation = Object.hasOwn(MEMBER_DEVIATIONS, key);
      assert.equal(member in instance, !deviation, deviation ? `${key} is listed as a deviation but exists` : `${key} is missing`);
    }
  }
  for (const key of Object.keys(MEMBER_DEVIATIONS)) {
    const [className, member] = key.split(".");
    assert.ok(V12_MEMBERS[className].includes(member), `${key} isn't a v12 member`);
  }
});

test("patterns have no parent setter, as DESIGN §22 says", () => {
  for (const [className, make] of Object.entries(INSTANCES)) {
    if (className === "Cursor") {
      continue;
    }
    const pattern = make();
    assert.throws(() => {
      "use strict";
      pattern.parent = null;
    }, TypeError, className);
  }
});

test("DESIGN §22 names every member deviation", () => {
  const section = designSection22();
  for (const key of [...Object.keys(MEMBER_DEVIATIONS), SETTER_DEVIATION]) {
    assert.ok(section.includes(key), `DESIGN §22 doesn't mention ${key}`);
  }
});
