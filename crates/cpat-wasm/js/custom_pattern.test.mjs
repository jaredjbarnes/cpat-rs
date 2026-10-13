// Custom patterns written in JS (DESIGN §9, §22), used directly and handed
// in by a grammar decorator; JS decorators that change a pattern in place or
// return its replacement. Calls are counted and recorded in order: parsing
// is synchronous, so nothing here waits on anything.
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  AutoComplete,
  CustomPattern,
  Expression,
  Grammar,
  Literal,
  Regex,
  Sequence,
  createPatternsTemplate,
  isTokenizablePattern,
} from "./index.mjs";

/** `(text, pos) => end | null` taking a run of ASCII digits, recording each
 * call as `[text, pos]`. */
function digits(calls = []) {
  const match = (text, pos) => {
    calls.push([text, pos]);
    let end = pos;
    while (end < text.length && text[end] >= "0" && text[end] <= "9") {
      end++;
    }
    return end > pos ? end : null;
  };
  return { match, calls };
}

// ─── Direct use ─────────────────────────────────────────────────────────

test("an answered end makes a custom node with the text it took, in UTF-16 offsets", () => {
  const { match, calls } = digits();
  const pattern = new Sequence("s", [new Literal("prefix", "é😀"), new CustomPattern("digits", match)]);

  const { ast } = pattern.exec("é😀12");
  const node = ast.children[1];

  assert.deepEqual([node.type, node.name, node.value, node.startIndex, node.endIndex], ["custom", "digits", "12", 3, 5]);
  // "é" is one unit and "😀" two, so the function is asked at 3, not at byte 6.
  assert.deepEqual(calls, [["é😀12", 3]]);
});

test("null is a failure, recorded where the function was asked", () => {
  const { match } = digits();
  const pattern = new Sequence("s", [new Literal("prefix", "é😀"), new CustomPattern("digits", match)]);

  const { ast, cursor } = pattern.exec("é😀x");

  assert.equal(ast, null);
  const furthest = cursor.furthestError;
  assert.deepEqual([furthest.pattern.name, furthest.startIndex, furthest.lastIndex], ["digits", 3, 3]);
});

test("a custom pattern fails where a regex for the same text fails", () => {
  const custom = new Sequence("s", [new Literal("prefix", "é😀"), new CustomPattern("digits", digits().match)]);
  const regex = new Sequence("s", [new Literal("prefix", "é😀"), new Regex("digits", "\\d+")]);
  const errors = pattern =>
    ["é😀12", "é😀x", "é😀", "x"].map(text => {
      const { cursor } = pattern.exec(text);
      const at = e => (e == null ? null : [e.pattern.name, e.startIndex, e.lastIndex]);
      return [at(cursor.error), at(cursor.furthestError)];
    });

  assert.deepEqual(errors(custom), errors(regex));
});

test("answering the position itself takes nothing and succeeds", () => {
  const nothing = new CustomPattern("nothing", (_text, pos) => pos);
  const pattern = new Sequence("s", [new Literal("a", "a"), nothing, new Literal("b", "b")]);

  const { ast } = pattern.exec("ab");

  assert.deepEqual(ast.children.map(n => n.name), ["a", "b"]);
});

test("the function sees each parse's whole text, and each attempt is one call", () => {
  const { match, calls } = digits();
  const list = new Sequence("list", [
    new CustomPattern("a", match),
    new Literal("comma", ","),
    new CustomPattern("b", match),
  ]);

  list.exec("1,22");
  list.exec("333,4");

  assert.deepEqual(calls, [
    ["1,22", 0],
    ["1,22", 2],
    ["333,4", 0],
    ["333,4", 4],
  ]);
});

test("a function that parses again (with a copy of itself) still sees its own parse's text", () => {
  const calls = [];
  let inner = null;
  const word = new CustomPattern("word", (text, pos) => {
    calls.push([text, pos]);
    if (text === "ab,cd" && pos === 0) {
      // Another parse, through a copy that shares this function, mid-attempt.
      assert.equal(inner.exec("zz").ast.value, "zz");
    }
    let end = pos;
    while (end < text.length && text[end] !== ",") {
      end++;
    }
    return end > pos ? end : null;
  });
  inner = word.clone();
  const items = new Sequence("items", [word, new Literal("comma", ","), word]);

  assert.equal(items.exec("ab,cd").ast.value, "ab,cd");
  assert.deepEqual(calls, [
    ["ab,cd", 0],
    ["zz", 0],
    ["ab,cd", 3],
  ]);
});

test("an answer longer than the text a nested parse saw is checked against its own text", () => {
  let inner = null;
  const word = new CustomPattern("word", (text, pos) => {
    if (text === "abcdef" && pos === 0) {
      assert.equal(inner.exec("z").ast.value, "z");
    }
    let end = pos;
    while (end < text.length && text[end] !== ",") {
      end++;
    }
    return end > pos ? end : null;
  });
  inner = word.clone();

  const { ast, cursor } = word.exec("abcdef");

  assert.equal(ast.value, "abcdef");
  assert.equal(cursor.hasError, false);
});

test("an answer is not judged by a surrogate pair in the text a nested parse saw", () => {
  let inner = null;
  const whole = new CustomPattern("whole", (text, pos) => {
    if (text === "ab" && pos === 0) {
      // In "a😀", offset 2 splits the pair; in "ab" it is the end.
      assert.equal(inner.exec("a\u{1F600}").ast.value, "a\u{1F600}");
    }
    return text.length;
  });
  inner = whole.clone();

  assert.equal(whole.exec("ab").ast.value, "ab");
});

test("a throw stops the parse: exec and test throw its message, and no alternative is tried", () => {
  const tried = [];
  const broken = new CustomPattern("broken", () => {
    tried.push("broken");
    throw new Error("boom");
  });
  const fallback = new CustomPattern("fallback", (_text, pos) => {
    tried.push("fallback");
    return pos + 1;
  });
  const either = new Expression("either", [broken, fallback]);

  assert.throws(() => either.exec("x"), { message: "custom pattern `broken` failed: boom" });
  assert.throws(() => either.test("x"), { message: "custom pattern `broken` failed: boom" });
  assert.deepEqual(tried, ["broken", "broken"]);
});

test("an answer that isn't null or an end in the text stops the parse and says what to return", () => {
  const answering = answer => new CustomPattern("odd", () => answer);
  const cases = [
    [99, "x", "answered 99 at 0"],
    [-1, "x", "answered -1 at 0"],
    [1.5, "xy", "answered 1.5 at 0"],
    ["1", "x", 'answered "1" at 0'],
    // Inside the surrogate pair of "😀".
    [1, "😀", "answered 1 at 0"],
  ];
  for (const [answer, text, said] of cases) {
    assert.throws(() => answering(answer).exec(text), error => {
      assert.ok(error.message.startsWith(`custom pattern \`odd\` failed: ${said};`), error.message);
      assert.ok(error.message.includes("return null, or an end from 0 to"), error.message);
      return true;
    });
  }
});

test("its constructor refuses something that isn't a function", () => {
  assert.throws(() => new CustomPattern("x", "not a function"), {
    name: "TypeError",
    message: /match must be a function \(text, pos\) => end \| null/,
  });
});

test("it takes tokens, which AutoComplete offers", () => {
  const word = new CustomPattern("word", digits().match);
  assert.ok(isTokenizablePattern(word));

  word.setTokens(["42", "7"]);
  const { options } = new AutoComplete(word).suggestFor("");

  assert.deepEqual(options.map(o => [o.text, o.startIndex, o.suggestionSequence[0].pattern.type]), [
    ["42", 0, "custom"],
    ["7", 0, "custom"],
  ]);
});

test("a copy, a child place and a clone are CustomPatterns that run the same function", () => {
  const { match, calls } = digits();
  const custom = new CustomPattern("n", match);
  const parent = new Sequence("parent", [custom]);

  const child = parent.children[0];
  const renamed = custom.clone("m");

  assert.ok(child instanceof CustomPattern);
  assert.equal(child.type, "custom");
  assert.equal(renamed.exec("5").ast.name, "m");
  assert.equal(parent.exec("6").ast.children[0].value, "6");
  assert.deepEqual(calls.map(c => c[0]), ["5", "6"]);
});

// ─── Through a grammar's decorators ─────────────────────────────────────

test("a decorator that returns a CustomPattern: the grammar parses through the JS function", () => {
  const calls = [];
  const custom = pattern =>
    new CustomPattern(pattern.name, (text, pos) => {
      calls.push(pos);
      let end = pos;
      while (end < text.length && /[a-zé]/.test(text[end])) {
        end++;
      }
      return end > pos ? end : null;
    });
  const { items } = createPatternsTemplate({ decorators: { custom } })`
    @custom
    word = /never/
    items = word + "," + word
  `;

  const { ast } = items.exec("été,ab");

  assert.deepEqual(ast.children.map(n => [n.type, n.name, n.value]), [
    ["custom", "word", "été"],
    ["literal", ",", ","],
    ["custom", "word", "ab"],
  ]);
  assert.deepEqual(calls, [0, 4]);
  // `/never/` is gone: nothing it would match is offered or parsed.
  assert.equal(items.exec("never,never").ast.value, "never,never");
  assert.deepEqual(calls, [0, 4, 0, 6]);
});

test("a decorator can change the pattern in place with setTokens, and returning nothing keeps it", () => {
  const { word } = createPatternsTemplate({
    decorators: { suggest: (pattern, words) => pattern.setTokens(words) },
  })`
    @suggest(["alpha", "beta"])
    word = /[a-z]+/
  `;

  assert.deepEqual(word.getTokens(), ["alpha", "beta"]);
  assert.equal(word.exec("gamma").ast.type, "regex");
});

test("a decorator's replacement must keep the rule's name", () => {
  const rename = () => new Literal("other", "x");

  assert.throws(() => Grammar.parseString(`@rename\nword = "x"`, { decorators: { rename } }), {
    message:
      "[Invalid Grammar] `@rename` renamed `word` to `other`; a decorator must keep the pattern's name, since other rules find it by that name.",
  });
});

test("a decorator that throws is an invalid grammar, with its message", () => {
  const strict = (pattern, arg) => {
    throw new Error(`\`@strict\` on \`${pattern.name}\` wants a number, got ${JSON.stringify(arg)}`);
  };

  assert.throws(() => Grammar.parseString(`@strict("x")\nword = "x"`, { decorators: { strict } }), {
    message: '[Invalid Grammar] `@strict` on `word` wants a number, got "x"',
  });
});

test("a value a decorator returns that isn't a pattern is ignored, as v12 ignores it", () => {
  const seen = [];
  const { word } = createPatternsTemplate({ decorators: { note: (_p, arg) => seen.push(arg) } })`
    @note(1)
    word = "x"
  `;

  assert.deepEqual(seen, [1]);
  assert.equal(word.exec("x").ast.value, "x");
});
