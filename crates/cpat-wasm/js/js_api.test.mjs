// What the JS API adds on top of the wasm layer (DESIGN §22): patterns as
// places in a root, the cursor's report, direct parses, decorators and
// tokens from JS, and an asynchronous build fetching ahead.
import { test } from "node:test";
import assert from "node:assert/strict";
import {
  Block,
  Context,
  Cursor,
  Cut,
  Expression,
  Grammar,
  Literal,
  Node,
  Not,
  Optional,
  Reference,
  Regex,
  Repeat,
  RightAssociated,
  Sequence,
  TakeUntil,
  createPatternsTemplate,
  isTokenizablePattern,
  patterns,
} from "./index.mjs";

function greeting() {
  return new Sequence("greeting", [
    new Literal("hello", "Hello"),
    new Regex("space", "\\s+"),
    new Literal("world", "World"),
  ]);
}

test("a child is a place in its parent: it knows its parent and what follows it", () => {
  const g = greeting();
  const [hello, space, world] = g.children;

  assert.deepEqual([hello.name, space.type, world.token], ["hello", "regex", "World"]);
  assert.equal(space.parent.name, "greeting");
  assert.equal(g.parent, null);
  assert.deepEqual(hello.getNextTokens(), []);
  assert.deepEqual(space.getNextTokens(), ["World"]);
  assert.deepEqual(g.getTokensAfter(space), ["World"]);
  assert.deepEqual(g.getPatternsAfter(space).map(p => p.name), ["world"]);
  assert.deepEqual(g.getPatterns().map(p => p.name), ["hello"]);
  // The children are the same objects each time, as v12's array is.
  assert.equal(g.children[1], space);
});

test("a pattern that isn't this one's child has nothing after it", () => {
  const g = greeting();
  const stranger = new Literal("world", "World");

  assert.deepEqual(g.getTokensAfter(stranger), []);
  assert.deepEqual(g.getTokensAfter(g.children[2].children[0] ?? g), []);
});

test("composites take copies: changing the original afterwards changes nothing", () => {
  const space = new Regex("space", "\\s+");
  const pair = new Sequence("pair", [new Literal("a", "a"), space, new Literal("b", "b")]);
  space.setTokens([" "]);

  assert.deepEqual(space.getTokens(), [" "]);
  assert.deepEqual(pair.children[1].getTokens(), []);
});

test("tokens are set on a pattern held, not on a child place", () => {
  const g = greeting();

  assert.ok(isTokenizablePattern(g.children[1]));
  assert.throws(() => g.children[1].setTokens([" "]), /is a child of another pattern/);
  assert.equal(isTokenizablePattern(g.children[0]), false);
});

test("find hands the predicate each candidate and returns a place in the root", () => {
  const g = greeting();
  const seen = [];
  const found = g.find(p => {
    seen.push(p.name);
    return p.name === "space";
  });

  assert.deepEqual(seen, ["hello", "space"]);
  assert.equal(found.name, "space");
  assert.equal(found.parent.name, "greeting");
  assert.equal(g.find(p => p.name === "nothing"), null);
});

test("a predicate that throws stops find and its error comes out as thrown", () => {
  const g = greeting();
  const boom = new Error("boom");
  let calls = 0;

  assert.throws(
    () =>
      g.find(() => {
        calls += 1;
        throw boom;
      }),
    error => error === boom
  );
  assert.equal(calls, 1);
  // The root is still usable afterwards.
  assert.equal(g.exec("Hello World").ast.value, "Hello World");
});

test("exec on a child place runs that pattern on its own", () => {
  const g = greeting();
  const { ast, cursor } = g.children[2].exec("World");

  assert.equal(ast.name, "world");
  assert.equal(cursor.hasError, false);
  assert.equal(g.children[2].test("Hello"), false);
});

// Expected values checked against v12 on the same pattern and text.
test("a recording exec reports every error and match, each naming its pattern", () => {
  const g = greeting();
  const { ast, cursor } = g.exec("Hello Wor", true);

  assert.equal(ast, null);
  assert.equal(cursor.isRecording, true);
  assert.deepEqual(
    cursor.records.map(r => (r.error == null ? `match ${r.pattern.name}` : `error ${r.pattern.name}`)),
    ["match hello", "match space", "error world", "error greeting"]
  );
  assert.deepEqual(cursor.allMatchedNodes.map(n => n.value), ["Hello", " "]);
  assert.equal(cursor.leafMatch.pattern.name, "space");
  assert.equal(cursor.leafMatch.node.lastIndex, 5);
  assert.deepEqual(
    cursor.errors.map(e => [e.pattern.name, e.startIndex, e.lastIndex]),
    [
      ["world", 6, 8],
      ["greeting", 0, 9],
    ]
  );
  assert.equal(cursor.furthestError.pattern.name, "world");
});

test("an ordinary exec keeps no paths for its errors, so they name their pattern only", () => {
  const { cursor } = greeting().exec("Hello Wor");

  assert.equal(cursor.isRecording, false);
  assert.deepEqual(cursor.errors, []);
  assert.equal(cursor.error.pattern.name, "greeting");
  assert.equal(typeof cursor.error.pattern.id, "string");
});

test("parse(cursor) parses from the cursor's index and leaves it on the match's last index", () => {
  const g = greeting();
  const hello = g.children[0];
  const cursor = new Cursor("Hello World");
  cursor.startRecording();

  const node = hello.parse(cursor);
  assert.ok(node instanceof Node);
  assert.deepEqual([node.value, cursor.index], ["Hello", 4]);
  assert.equal(cursor.isRecording, true);

  cursor.moveTo(6);
  const world = g.children[2].parse(cursor);
  assert.deepEqual([world.value, world.firstIndex, cursor.index], ["World", 6, 10]);

  cursor.moveTo(0);
  assert.equal(g.children[2].parse(cursor), null);
  assert.equal(cursor.error.pattern.name, "world");
});

test("input nested too deep is an answer on the cursor", () => {
  const { expr } = patterns`
    expr = "(" + expr + ")" | "x"
  `;
  const deep = "(".repeat(2000) + "x" + ")".repeat(2000);
  const { ast, cursor } = expr.exec(deep);

  assert.equal(ast, null);
  assert.equal(cursor.tooDeep.length, 2);
  assert.equal(expr.exec("((x))").ast.value, "((x))");
});

test("a JS decorator gets the rule's pattern and JSON argument, and its changes are kept", () => {
  const seen = [];
  const build = createPatternsTemplate({
    decorators: {
      suggest: (pattern, arg) => {
        seen.push([pattern.name, arg]);
        pattern.setTokens(arg.tokens);
      },
    },
  });
  const { word } = build`
    @suggest({"tokens": ["yes", "no"]})
    word = /[a-z]+/
  `;

  assert.deepEqual(seen, [["word", { tokens: ["yes", "no"] }]]);
  assert.deepEqual(word.getTokens(), ["yes", "no"]);
  assert.equal(word.exec("maybe").ast.value, "maybe");
});

test("a decorator may keep the pattern it was handed; it stays usable", () => {
  const kept = [];
  createPatternsTemplate({ decorators: { keep: p => kept.push(p) } })`
    @keep
    word = /[a-z]+/
  `;

  assert.equal(kept[0].exec("abc").ast.value, "abc");
});

test("params are JS patterns, copied into the build", () => {
  const value = new Literal("value", "v");
  const { x } = Grammar.parseString(`use params { value }\nx = value + "!"`, { params: [value] });

  assert.equal(x.exec("v!").ast.value, "v!");
  assert.equal(value.exec("v").ast.value, "v");
});

test("an async build answers each import statement with its own fetch, in order", async () => {
  let call = 0;
  const p = await Grammar.parse(
    `
      import { v as first } from "counter"
      import { v as second } from "counter"
      x = first + second
    `,
    {
      resolveImport: resource => {
        call += 1;
        return Promise.resolve({ expression: `v = "${call}"`, resource });
      },
    }
  );

  assert.equal(call, 2);
  assert.equal(p.x.exec("12").ast.value, "12");
});

test("a context lists its definitions by name", () => {
  const context = new Context("ctx", new Reference("space"), [new Literal("space", " ")]);
  const within = context.getPatternsWithinContext();

  assert.deepEqual(Object.keys(within), ["space"]);
  assert.equal(within.space.token, " ");
  assert.equal(context.getPatternWithinContext("ctx"), context);
  assert.equal(context.exec(" ").ast.value, " ");
});

test("every pattern class builds with v12's constructor signature", () => {
  const word = new Regex("word", "[a-z]+");
  const block = new Block("block", new Literal("open", "{"), null, new Literal("close", "}"));
  const until = new TakeUntil("until", new Literal("end", ";"));
  const ops = new Expression("expr", [
    new RightAssociated(new Sequence("pow", [new Reference("expr"), new Literal("^", "^"), new Reference("expr")])),
    new Regex("num", "\\d+"),
  ]);
  const list = new Repeat("list", word, { divider: new Literal(",", ","), min: 2, max: 3, trimDivider: true });

  assert.deepEqual(
    [new Cut().name, new Cut("commit").name, new Cut().type],
    ["cut", "commit", "cut"]
  );
  assert.equal(new Optional("maybe", word).exec("").ast, null);
  assert.equal(new Optional("maybe", word).test("abc"), true);
  assert.equal(new Not("not-word", word).type, "not");
  assert.deepEqual([block.type, block.exec("{a{b}}").ast.value], ["block", "{a{b}}"]);
  assert.deepEqual([until.type, until.exec("abc").ast.value], ["take-until", "abc"]);
  assert.equal(ops.exec("2^3^4").ast.children[2].value, "3^4");
  assert.deepEqual([list.type, list.test("a,b,c"), list.test("a"), list.test("a,b,c,d")], [
    "finite-repeat",
    true,
    false,
    false,
  ]);
  assert.equal(new Repeat("words", word).type, "infinite-repeat");
});

// v12's `test` is `!cursor.hasError`, and its exec returns before parsing
// empty text, so `test("")` is true for every pattern while `exec("")` has
// no ast and no error. Values checked against v12 on the same patterns.
test("test on empty text passes for every pattern, as exec reports no error", () => {
  const a = () => new Literal("a", "a");
  const g = greeting();
  const all = [
    a(),
    new Regex("digits", "\\d+"),
    new Repeat("as", a(), { min: 1 }),
    new Sequence("s", [a()]),
    new Optional("maybe", a()),
    new Not("not-a", a()),
    g,
    g.children[2],
    Grammar.parseString('a = "a"\nb = a').b,
  ];

  for (const pattern of all) {
    assert.equal(pattern.test(""), true, pattern.name);
    assert.equal(pattern.test("", true), true, pattern.name);
    const { ast, cursor } = pattern.exec("");
    assert.equal(ast, null, pattern.name);
    assert.equal(cursor.hasError, false, pattern.name);
  }
  assert.equal(a().test("b"), false);
  assert.equal(a().test("a"), true);
});

test("test throws what exec throws when a reference names nothing, as v12's does", () => {
  const dangling = new Reference("x");

  assert.throws(() => dangling.test("x"), { message: "Couldn't find 'x' pattern within tree." });
  assert.throws(() => dangling.test("x", true), {
    message: "Couldn't find 'x' pattern within tree.",
  });
  assert.throws(() => dangling.exec("x"), { message: "Couldn't find 'x' pattern within tree." });
  const holder = new Sequence("s", [new Reference("x")]);
  assert.throws(() => holder.children[0].test("x"), {
    message: "Couldn't find 'x' pattern within tree.",
  });
});
