// Behavior compared with clarity-pattern-parser v12 by running the same
// cases through both (tsx against v12's src for V12): the order an
// asynchronous build asks the resolver in, AutoComplete's answers in UTF-16
// offsets, and v12's Node API on a tree from toNode().
import { test } from "node:test";
import assert from "node:assert/strict";
import * as api from "./index.mjs";

const V12 = {"resolverCalls":{"calls":[["root.cpat",null],["a.cpat","/root.cpat"],["b.cpat","/a.cpat"],["c.cpat","/a.cpat"],["c.cpat","/root.cpat"]],"matched":"bb d"},"autoComplete":[{"text":"","isComplete":false,"options":[["😀",0,[["😀","😀"]]],["Jane",0,[["Jane","Jane"]]],["John",0,[["John","John"]]]],"error":[0,0],"errorAtIndex":0,"ast":null},{"text":"😀","isComplete":false,"options":[[" likes",2,[[" ","space"],["likes","likes"]]],[" loves",2,[[" ","space"],["loves","loves"]]]],"error":[2,2],"errorAtIndex":2,"ast":null},{"text":"John l","isComplete":false,"options":[["ikes",6,[["likes","likes"]]],["oves",6,[["loves","loves"]]]],"error":[5,5],"errorAtIndex":5,"ast":null},{"text":"Jane loves 😀","isComplete":true,"options":[],"error":null,"errorAtIndex":null,"ast":"Jane loves 😀"},{"text":"Jane x","isComplete":false,"options":[["likes",5,[["likes","likes"]]],["loves",5,[["loves","loves"]]]],"error":[5,5],"errorAtIndex":5,"ast":null}],"nodeOps":{"tree":"{\"type\":\"expression\",\"name\":\"add\",\"value\":\"1 + -2 * (3+4)\",\"startIndex\":0,\"endIndex\":14,\"children\":[{\"type\":\"regex\",\"name\":\"num\",\"value\":\"1\",\"startIndex\":0,\"endIndex\":1,\"children\":[]},{\"type\":\"regex\",\"name\":\"space\",\"value\":\" \",\"startIndex\":1,\"endIndex\":2,\"children\":[]},{\"type\":\"literal\",\"name\":\"+\",\"value\":\"+\",\"startIndex\":2,\"endIndex\":3,\"children\":[]},{\"type\":\"regex\",\"name\":\"space\",\"value\":\" \",\"startIndex\":3,\"endIndex\":4,\"children\":[]},{\"type\":\"expression\",\"name\":\"mul\",\"value\":\"-2 * (3+4)\",\"startIndex\":4,\"endIndex\":14,\"children\":[{\"type\":\"expression\",\"name\":\"neg\",\"value\":\"-2\",\"startIndex\":4,\"endIndex\":6,\"children\":[{\"type\":\"literal\",\"name\":\"-\",\"value\":\"-\",\"startIndex\":4,\"endIndex\":5,\"children\":[]},{\"type\":\"regex\",\"name\":\"num\",\"value\":\"2\",\"startIndex\":5,\"endIndex\":6,\"children\":[]}]},{\"type\":\"regex\",\"name\":\"space\",\"value\":\" \",\"startIndex\":6,\"endIndex\":7,\"children\":[]},{\"type\":\"literal\",\"name\":\"*\",\"value\":\"*\",\"startIndex\":7,\"endIndex\":8,\"children\":[]},{\"type\":\"regex\",\"name\":\"space\",\"value\":\" \",\"startIndex\":8,\"endIndex\":9,\"children\":[]},{\"type\":\"sequence\",\"name\":\"group\",\"value\":\"(3+4)\",\"startIndex\":9,\"endIndex\":14,\"children\":[{\"type\":\"literal\",\"name\":\"(\",\"value\":\"(\",\"startIndex\":9,\"endIndex\":10,\"children\":[]},{\"type\":\"expression\",\"name\":\"add\",\"value\":\"3+4\",\"startIndex\":10,\"endIndex\":13,\"children\":[{\"type\":\"regex\",\"name\":\"num\",\"value\":\"3\",\"startIndex\":10,\"endIndex\":11,\"children\":[]},{\"type\":\"literal\",\"name\":\"+\",\"value\":\"+\",\"startIndex\":11,\"endIndex\":12,\"children\":[]},{\"type\":\"regex\",\"name\":\"num\",\"value\":\"4\",\"startIndex\":12,\"endIndex\":13,\"children\":[]}]},{\"type\":\"literal\",\"name\":\")\",\"value\":\")\",\"startIndex\":13,\"endIndex\":14,\"children\":[]}]}]}]}","walkUp":["num","space","+","space","-","num","neg","space","*","space","(","num","+","num","add",")","group","mul","add"],"walkDown":["add","num","space","+","space","mul","neg","-","num","space","*","space","group","(","add","num","+","num",")"],"breadthFirst":["add","num","space","+","space","mul","neg","space","*","space","group","-","num","(","add",")","num","+","num"],"findQuirk":"num","findAllNums":[["1",0,0],["2",5,5],["3",10,10],["4",12,12]],"flatten":["1"," ","+"," ","-","2"," ","*"," ","(","3","+","4",")"],"cloneEqual":true,"removedSpaces":"{\"type\":\"expression\",\"name\":\"add\",\"value\":\"1+-2*(3+4)\",\"startIndex\":0,\"endIndex\":10,\"children\":[{\"type\":\"regex\",\"name\":\"num\",\"value\":\"1\",\"startIndex\":0,\"endIndex\":1,\"children\":[]},{\"type\":\"literal\",\"name\":\"+\",\"value\":\"+\",\"startIndex\":1,\"endIndex\":2,\"children\":[]},{\"type\":\"expression\",\"name\":\"mul\",\"value\":\"-2*(3+4)\",\"startIndex\":2,\"endIndex\":10,\"children\":[{\"type\":\"expression\",\"name\":\"neg\",\"value\":\"-2\",\"startIndex\":2,\"endIndex\":4,\"children\":[{\"type\":\"literal\",\"name\":\"-\",\"value\":\"-\",\"startIndex\":2,\"endIndex\":3,\"children\":[]},{\"type\":\"regex\",\"name\":\"num\",\"value\":\"2\",\"startIndex\":3,\"endIndex\":4,\"children\":[]}]},{\"type\":\"literal\",\"name\":\"*\",\"value\":\"*\",\"startIndex\":4,\"endIndex\":5,\"children\":[]},{\"type\":\"sequence\",\"name\":\"group\",\"value\":\"(3+4)\",\"startIndex\":5,\"endIndex\":10,\"children\":[{\"type\":\"literal\",\"name\":\"(\",\"value\":\"(\",\"startIndex\":5,\"endIndex\":6,\"children\":[]},{\"type\":\"expression\",\"name\":\"add\",\"value\":\"3+4\",\"startIndex\":6,\"endIndex\":9,\"children\":[{\"type\":\"regex\",\"name\":\"num\",\"value\":\"3\",\"startIndex\":6,\"endIndex\":7,\"children\":[]},{\"type\":\"literal\",\"name\":\"+\",\"value\":\"+\",\"startIndex\":7,\"endIndex\":8,\"children\":[]},{\"type\":\"regex\",\"name\":\"num\",\"value\":\"4\",\"startIndex\":8,\"endIndex\":9,\"children\":[]}]},{\"type\":\"literal\",\"name\":\")\",\"value\":\")\",\"startIndex\":9,\"endIndex\":10,\"children\":[]}]}]}]}","compacted":"{\"type\":\"expression\",\"name\":\"add\",\"value\":\"1 + -2 * (3+4)\",\"startIndex\":0,\"endIndex\":14,\"children\":[{\"type\":\"regex\",\"name\":\"num\",\"value\":\"1\",\"startIndex\":0,\"endIndex\":1,\"children\":[]},{\"type\":\"regex\",\"name\":\"space\",\"value\":\" \",\"startIndex\":1,\"endIndex\":2,\"children\":[]},{\"type\":\"literal\",\"name\":\"+\",\"value\":\"+\",\"startIndex\":2,\"endIndex\":3,\"children\":[]},{\"type\":\"regex\",\"name\":\"space\",\"value\":\" \",\"startIndex\":3,\"endIndex\":4,\"children\":[]},{\"type\":\"expression\",\"name\":\"mul\",\"value\":\"-2 * (3+4)\",\"startIndex\":4,\"endIndex\":14,\"children\":[{\"type\":\"expression\",\"name\":\"neg\",\"value\":\"-2\",\"startIndex\":4,\"endIndex\":6,\"children\":[{\"type\":\"literal\",\"name\":\"-\",\"value\":\"-\",\"startIndex\":4,\"endIndex\":5,\"children\":[]},{\"type\":\"regex\",\"name\":\"num\",\"value\":\"2\",\"startIndex\":5,\"endIndex\":6,\"children\":[]}]},{\"type\":\"regex\",\"name\":\"space\",\"value\":\" \",\"startIndex\":6,\"endIndex\":7,\"children\":[]},{\"type\":\"literal\",\"name\":\"*\",\"value\":\"*\",\"startIndex\":7,\"endIndex\":8,\"children\":[]},{\"type\":\"regex\",\"name\":\"space\",\"value\":\" \",\"startIndex\":8,\"endIndex\":9,\"children\":[]},{\"type\":\"sequence\",\"name\":\"group\",\"value\":\"(3+4)\",\"startIndex\":9,\"endIndex\":14,\"children\":[]}]}]}","transformed":"1 + 2 * (3+4)","json":"{\"type\":\"expression\",\"name\":\"add\",\"value\":\"1 + -2 * (3+4)\",\"startIndex\":0,\"endIndex\":14,\"children\":[{\"type\":\"regex\",\"name\":\"num\",\"value\":\"1\",\"startIndex\":0,\"endIndex\":1,\"children\":[]},{\"type\":\"regex\",\"name\":\"space\",\"value\":\" \",\"startIndex\":1,\"endIndex\":2,\"children\":[]},{\"type\":\"literal\",\"name\":\"+\",\"value\":\"+\",\"startIndex\":2,\"endIndex\":3,\"children\":[]},{\"type\":\"regex\",\"name\":\"space\",\"value\":\" \",\"startIndex\":3,\"endIndex\":4,\"children\":[]},{\"type\":\"expression\",\"name\":\"mul\",\"value\":\"-2 * (3+4)\",\"startIndex\":4,\"endIndex\":14,\"children\":[{\"type\":\"expression\",\"name\":\"neg\",\"value\":\"-2\",\"startIndex\":4,\"endIndex\":6,\"children\":[{\"type\":\"literal\",\"name\":\"-\",\"value\":\"-\",\"startIndex\":4,\"endIndex\":5,\"children\":[]},{\"type\":\"regex\",\"name\":\"num\",\"value\":\"2\",\"startIndex\":5,\"endIndex\":6,\"children\":[]}]},{\"type\":\"regex\",\"name\":\"space\",\"value\":\" \",\"startIndex\":6,\"endIndex\":7,\"children\":[]},{\"type\":\"literal\",\"name\":\"*\",\"value\":\"*\",\"startIndex\":7,\"endIndex\":8,\"children\":[]},{\"type\":\"regex\",\"name\":\"space\",\"value\":\" \",\"startIndex\":8,\"endIndex\":9,\"children\":[]},{\"type\":\"sequence\",\"name\":\"group\",\"value\":\"(3+4)\",\"startIndex\":9,\"endIndex\":14,\"children\":[{\"type\":\"literal\",\"name\":\"(\",\"value\":\"(\",\"startIndex\":9,\"endIndex\":10,\"children\":[]},{\"type\":\"expression\",\"name\":\"add\",\"value\":\"3+4\",\"startIndex\":10,\"endIndex\":13,\"children\":[{\"type\":\"regex\",\"name\":\"num\",\"value\":\"3\",\"startIndex\":10,\"endIndex\":11,\"children\":[]},{\"type\":\"literal\",\"name\":\"+\",\"value\":\"+\",\"startIndex\":11,\"endIndex\":12,\"children\":[]},{\"type\":\"regex\",\"name\":\"num\",\"value\":\"4\",\"startIndex\":12,\"endIndex\":13,\"children\":[]}]},{\"type\":\"literal\",\"name\":\")\",\"value\":\")\",\"startIndex\":13,\"endIndex\":14,\"children\":[]}]}]}]}"}};

function cases(api) {
  const { Grammar, AutoComplete, patterns, compact, remove, Literal } = api;
  const strip = o => ({ ...o, id: undefined, children: o.children.map(strip) });
  const describeNode = n => n == null ? null : JSON.stringify(strip(n.toCycleFreeObject()));
  return {
    // The order (and origins) v12's async build asks the resolver in.
    async resolverCalls() {
      const files = {
        "root.cpat": `use params { sp }\nimport { a } from "a.cpat"\nimport { c, d } from "c.cpat" with params { p = sp }\nx = a + c + d`,
        "a.cpat": `import { b } from "b.cpat"\nimport { c } from "c.cpat" with params { p = b }\na = b + c`,
        "b.cpat": `b = "b"`,
        "c.cpat": `use params { p }\nc = p\nd = "d"`,
      };
      const calls = [];
      const resolveImport = (resource, origin) => {
        calls.push([resource, origin]);
        return Promise.resolve({ expression: files[resource], resource: "/" + resource });
      };
      const fixed = (resource, origin) => resolveImport(resource.replace(/^\//, ""), origin);
      const built = await Grammar.import("root.cpat", { resolveImport: fixed, params: [new Literal("sp", " ")] });
      return { calls, matched: built.x.exec("bb d").ast?.value ?? null };
    },
    autoComplete() {
      const { statement } = patterns`
        name = "😀" | "John" | "Jane"
        space = " "
        verb = "likes" | "loves"
        statement = name + space + verb + space + name
      `;
      const ac = new AutoComplete(statement, { greedyPatternNames: ["space"] });
      return ["", "😀", "John l", "Jane loves 😀", "Jane x"].map(text => {
        const s = ac.suggestFor(text);
        return {
          text,
          isComplete: s.isComplete,
          options: s.options.map(o => [o.text, o.startIndex, o.suggestionSequence.map(seg => [seg.text, seg.pattern.name])]),
          error: s.error == null ? null : [s.error.startIndex, s.error.lastIndex],
          errorAtIndex: s.errorAtIndex,
          ast: s.ast == null ? null : s.ast.value,
        };
      });
    },
    // v12's Node API on a parse tree.
    nodeOps() {
      const { expr } = patterns`
        num = /\\d+/
        space = /\\s+/
        op = "+" | "*"
        add = expr + space? + "+" + space? + expr
        mul = expr + space? + "*" + space? + expr
        neg = "-" + expr
        group = "(" + expr + ")"
        expr = neg | mul | add | group | num
      `;
      const ast = expr.exec("1 + -2 * (3+4)").ast;
      const out = {};
      out.tree = describeNode(ast);
      const up = [];
      ast.walkUp(n => { up.push(n.name); });
      out.walkUp = up;
      const down = [];
      ast.walkDown(n => { down.push(n.name); });
      out.walkDown = down;
      const bf = [];
      ast.walkBreadthFirst(n => { bf.push(n.name); });
      out.breadthFirst = bf;
      out.findQuirk = ast.find(n => n.name.startsWith("m") || n.name === "num")?.name;
      out.findAllNums = ast.findAll(n => n.name === "num").map(n => [n.value, n.firstIndex, n.lastIndex]);
      out.flatten = ast.flatten().map(n => n.value);
      out.cloneEqual = ast.clone().isEqual(ast);
      const edited = ast.clone();
      remove(edited, { space: true });
      edited.normalize();
      out.removedSpaces = describeNode(edited);
      const compacted = ast.clone();
      compact(compacted, { group: true });
      out.compacted = describeNode(compacted);
      const transformed = ast.clone().transform({ num: n => { const r = n.clone(); return r; }, neg: n => n.children[1] });
      out.transformed = transformed.value;
      out.json = (() => { const j = JSON.parse(ast.toJson(2)); const s = o => { delete o.id; o.children.forEach(s); return o; }; return JSON.stringify(s(j)); })();
      return out;
    },
  };
}

const run = cases(api);

test("an async build fetches imports in v12's order, from v12's origins", async () => {
  assert.deepEqual(await run.resolverCalls(), V12.resolverCalls);
});

test("AutoComplete answers as v12 does, in UTF-16 offsets", () => {
  const expected = structuredClone(V12.autoComplete);
  // Deviation (DESIGN §17): options are sorted as JS localeCompare sorts
  // them for the characters cpat has collation tables for; anything else
  // (an emoji) sorts after every letter, where ICU puts it first.
  const [emoji, ...rest] = expected[0].options;
  expected[0].options = [...rest, emoji];
  assert.deepEqual(run.autoComplete(), expected);
});

test("v12's Node API on a toNode() tree: walks, finds, edits, JSON", () => {
  assert.deepEqual(run.nodeOps(), V12.nodeOps);
});
