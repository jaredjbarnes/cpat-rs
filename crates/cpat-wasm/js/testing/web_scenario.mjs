// What the web-entry tests run once wasm is loaded: a grammar parsed through
// each part of the API. The expected values are v12's, from running the
// same calls against clarity-pattern-parser v12's src/index.ts.
import assert from "node:assert/strict";

function shape(node) {
  return {
    type: node.type,
    name: node.name,
    value: node.value,
    startIndex: node.startIndex,
    endIndex: node.endIndex,
    children: node.children.map(shape),
  };
}

const leaf = (type, name, value, startIndex) => ({
  type,
  name,
  value,
  startIndex,
  endIndex: startIndex + value.length,
  children: [],
});

export function parsesThroughTheApi(api) {
  const { patterns, Grammar, AutoComplete, grammar } = api;

  const { expr } = patterns`
    num = /\\d+/
    add = expr + "+" + expr
    mul = expr + "*" + expr
    expr = mul | add | num
  `;
  const { ast } = expr.exec("1+2*3");
  assert.deepEqual(shape(ast), {
    type: "expression",
    name: "add",
    value: "1+2*3",
    startIndex: 0,
    endIndex: 5,
    children: [
      leaf("regex", "num", "1", 0),
      leaf("literal", "+", "+", 1),
      {
        type: "expression",
        name: "mul",
        value: "2*3",
        startIndex: 2,
        endIndex: 5,
        children: [leaf("regex", "num", "2", 2), leaf("literal", "*", "*", 3), leaf("regex", "num", "3", 4)],
      },
    ],
  });

  const failed = expr.exec("1+");
  assert.equal(failed.ast, null);
  assert.deepEqual([failed.cursor.error.startIndex, failed.cursor.error.endIndex], [1, 3]);

  const { greeting } = Grammar.parseString('greeting = "Hello" + " " + name\nname = "World" | "You"');
  const hello = greeting.exec("Hello You").ast;
  assert.deepEqual(
    hello.findAll((n) => n.isLeaf).map((n) => `${n.name}:${n.value}`),
    ["Hello:Hello", " : ", "You:You"],
  );
  assert.deepEqual(
    new AutoComplete(greeting).suggestFor("Hello ").options.map((o) => o.text),
    ["World", "You"],
  );

  assert.equal(grammar.test('a = "b"'), true);
  assert.equal(grammar.test("a = "), false);
}
