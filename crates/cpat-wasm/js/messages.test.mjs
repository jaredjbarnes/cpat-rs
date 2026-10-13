// Error messages are v12's, character for character: every case below
// was run through clarity-pattern-parser v12 (tsx against its src) and its
// message copied into V12_MESSAGES. Grammar errors come from wasm as the
// core's GrammarError text; resolver and decorator failures cross back as
// their own messages and are wrapped as v12 wraps them.
import { test } from "node:test";
import assert from "node:assert/strict";
import * as api from "./index.mjs";

const V12_MESSAGES = {
  "syntax at start": "[Invalid Grammar] Error at line 1, column 1. Hint: syntax, use params, import, @, @",
  "syntax further in": "[Invalid Grammar] Error at line 2, column 23. Hint: !,?,(,(,[",
  "empty grammar": "[Invalid Grammar] Error at line 1, column 1. Hint: syntax, use params, import, @, @",
  "unknown pattern": "[Invalid Grammar] unknown pattern `missing` used in `x`; define it in this grammar, import it, or pass it as a param.",
  "unknown export": "[Invalid Grammar] unknown pattern `missing` used in `export missing`; define it in this grammar, import it, or pass it as a param.",
  "unknown use params default": "[Invalid Grammar] unknown pattern `nowhere` used in `use params { value }`; define it in this grammar, import it, or pass it as a param.",
  "alias cycle": "[Invalid Grammar] alias cycle `a` -> `b` -> `c` -> `a`: each rule only renames the next, so none of them matches anything.",
  "shadowed literal": "[Invalid Grammar] `\"<=\"` can never match after `\"<\"` in `x`; put the longer literal first.",
  "greedy options": "[Invalid Grammar] greedy options (`<|>`) were removed in v12; order longer alternatives first, or use a regex such as /==|=/. Found: `\"ab\" <|> \"abc\" <|> \"a\"` at line 1, column 8.",
  "tokens on a literal": "[Invalid Grammar] `@tokens` can't apply to `name` (a literal) — only patterns with setTokens accept tokens",
  "tokens without an array": "[Invalid Grammar] `@tokens` on `spaces` expects an array of strings",
  "unknown decorator": "[Invalid Grammar] unknown decorator `@bad-decorator` on `spaces`; known decorators: @tokens.",
  "a decorator that throws": "[Invalid Grammar] boom on x",
  "sync import without resolver": "No import resolver supplied.",
  "async import without resolver": "No import resolver supplied.",
  "sync import of a bad grammar": "Failed loading expression from: \"bad.cpat\". Error details: \"[Invalid Grammar] Error at line 1, column 1. Hint: syntax, use params, import, @, @\"",
  "async import of a bad grammar": "Failed loading expression from: \"bad.cpat\". Error details: \"[Invalid Grammar] Error at line 1, column 1. Hint: syntax, use params, import, @, @\"",
  "async resolver rejects": "no file nowhere.cpat",
  "async rejection two imports deep": "Failed loading expression from: \"a.cpat\". Error details: \"b is gone\"",
  "async name not in the import": "Failed loading expression from: \"missing-name.cpat\". Error details: \"Couldn't find pattern with name: nonexistent, from import: missing-name.cpat.\"",
  "async alias not in the import": "Failed loading expression from: \"missing-name.cpat\". Error details: \"Couldn't find pattern with name: nonexistent, from import: missing-name.cpat.\"",
  "async duplicate import": "Failed loading expression from: \"b.cpat\". Error details: \"'b' was already used within another import.\"",
  "async import whose param is missing": "Failed loading expression from: \"uses-param.cpat\". Error details: \"[Invalid Grammar] unknown pattern `p` used in `q`; define it in this grammar, import it, or pass it as a param.\"",
  "async Grammar.import rejects": "no file nowhere.cpat",
  "selector syntax": "[Invalid Selector] Error at line 1, column 3. Hint: ",
};

// Each case builds something that fails.
function cases(api) {
  const { Grammar, Literal, Selector, createPatternsTemplate } = api;
  const files = {
    "a.cpat": `import { b } from "b.cpat"\na = b`,
    "b.cpat": `b = "B"`,
    "bad.cpat": `///bad grammar///`,
    "missing-name.cpat": `x = "X"`,
    "uses-param.cpat": `use params { p }\nq = p + "!"`,
  };
  const resolveImport = r => (r in files ? Promise.resolve({ expression: files[r], resource: r }) : Promise.reject(new Error(`no file ${r}`)));
  const resolveImportSync = r => {
    if (!(r in files)) throw new Error(`no file ${r}`);
    return { expression: files[r], resource: r };
  };
  return {
    "syntax at start": () => Grammar.parseString(`//`),
    "syntax further in": () => Grammar.parseString(`name = /\\w/\n                age = /()\n            `),
    "empty grammar": () => Grammar.parseString(""),
    "unknown pattern": () => Grammar.parseString(`x = missing + "b"`),
    "unknown export": () => Grammar.parseString(`missing`),
    "unknown use params default": () => Grammar.parseString(`use params { value = nowhere }\nx = value`),
    "alias cycle": () => Grammar.parseString(`entry = a\na = b\nb = c\nc = a`),
    "shadowed literal": () => Grammar.parseString(`x = "a" + ("<" | ">" | "<=")`),
    "greedy options": () => Grammar.parseString(`item = "ab" <|> "abc" <|> "a"`),
    "tokens on a literal": () => Grammar.parseString(`@tokens(["ignored"])\nname = "John"`),
    "tokens without an array": () => Grammar.parseString(`@tokens("Bad")\nspaces = /\\s+/`),
    "unknown decorator": () => Grammar.parseString(`@bad-decorator()\nspaces = /\\s+/`),
    "a decorator that throws": () =>
      createPatternsTemplate({ decorators: { boom: p => { throw new Error(`boom on ${p.name}`); } } })`@boom\nx = /x/`,
    "sync import without resolver": () => Grammar.parseString(`import { value } from "file.cpat"\nname = value`),
    "async import without resolver": () => Grammar.parse(`import { value } from "file.cpat"\nname = value`),
    "sync import of a bad grammar": () => Grammar.parseString(`import { value } from "bad.cpat"\nname = value`, { resolveImportSync }),
    "async import of a bad grammar": () => Grammar.parse(`import { value } from "bad.cpat"\nname = value`, { resolveImport }),
    "async resolver rejects": () => Grammar.parse(`import { value } from "nowhere.cpat"\nname = value`, { resolveImport }),
    "async rejection two imports deep": () =>
      Grammar.parse(`import { a } from "a.cpat"\nx = a`, { resolveImport: r => (r === "b.cpat" ? Promise.reject(new Error("b is gone")) : resolveImport(r)) }),
    "async name not in the import": () => Grammar.parse(`import { nonexistent } from "missing-name.cpat"`, { resolveImport }),
    "async alias not in the import": () => Grammar.parse(`import { nonexistent as alias } from "missing-name.cpat"`, { resolveImport }),
    "async duplicate import": () => Grammar.parse(`import { b } from "b.cpat"\nimport { b } from "b.cpat"`, { resolveImport }),
    "async import whose param is missing": () => Grammar.parse(`import { q } from "uses-param.cpat"\nx = q`, { resolveImport }),
    "async Grammar.import rejects": () => Grammar.import("nowhere.cpat", { resolveImport }),
    "selector syntax": () => new Selector("*,["),
  };
}

for (const [name, run] of Object.entries(cases(api))) {
  test(`v12's message: ${name}`, async () => {
    let message = "(no error)";
    try {
      await run();
    } catch (error) {
      message = error.message;
    }
    assert.equal(message, V12_MESSAGES[name]);
  });
}
