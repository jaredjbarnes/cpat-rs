// Positions cross the wasm boundary in UTF-16 offsets, so a parse of text
// with emoji (two units each, one with a skin-tone modifier) and CJK gives
// the startIndex/endIndex/firstIndex/lastIndex and error positions v12 gives.
// EXPECTED was written once by running the same grammar and inputs through
// clarity-pattern-parser v12 (tsx against its src).
import { test } from "node:test";
import assert from "node:assert/strict";
import { patterns, generateErrorMessage } from "./index.mjs";

const grammar = `
emoji = "😀" | "🎉" | "👍🏽"
cjk = "你好" | "世界"
word = /[a-z]+/
space = " "
item = emoji | cjk | word
items = (item, space)+
`;

const EXPECTED = [
  {"text":"hi 😀 你好 🎉 世界 ok","record":false,"nodes":[["items","infinite-repeat",0,17,0,16,"hi 😀 你好 🎉 世界 ok"],["word","regex",0,2,0,1,"hi"],["space","literal",2,3,2,2," "],["😀","literal",3,5,3,4,"😀"],["space","literal",5,6,5,5," "],["你好","literal",6,8,6,7,"你好"],["space","literal",8,9,8,8," "],["🎉","literal",9,11,9,10,"🎉"],["space","literal",11,12,11,11," "],["世界","literal",12,14,12,13,"世界"],["space","literal",14,15,14,14," "],["word","regex",15,17,15,16,"ok"]],"error":null,"furthestError":["😀",15,16,15,15],"leafMatch":["word",15,17],"errors":[],"message":null},
  {"text":"hi 😀 你好 🎉 世界 ok","record":true,"nodes":[["items","infinite-repeat",0,17,0,16,"hi 😀 你好 🎉 世界 ok"],["word","regex",0,2,0,1,"hi"],["space","literal",2,3,2,2," "],["😀","literal",3,5,3,4,"😀"],["space","literal",5,6,5,5," "],["你好","literal",6,8,6,7,"你好"],["space","literal",8,9,8,8," "],["🎉","literal",9,11,9,10,"🎉"],["space","literal",11,12,11,11," "],["世界","literal",12,14,12,13,"世界"],["space","literal",14,15,14,14," "],["word","regex",15,17,15,16,"ok"]],"error":null,"furthestError":["😀",15,16,15,15],"leafMatch":["word",15,17],"errors":[["😀",0,1,0,0],["🎉",0,1,0,0],["👍🏽",0,1,0,0],["emoji",0,1,0,0],["你好",0,1,0,0],["世界",0,1,0,0],["cjk",0,1,0,0],["😀",6,7,6,6],["🎉",6,7,6,6],["👍🏽",6,7,6,6],["emoji",6,7,6,6],["😀",9,10,9,9],["😀",12,13,12,12],["🎉",12,13,12,12],["👍🏽",12,13,12,12],["emoji",12,13,12,12],["你好",12,13,12,12],["😀",15,16,15,15],["🎉",15,16,15,15],["👍🏽",15,16,15,15],["emoji",15,16,15,15],["你好",15,16,15,15],["世界",15,16,15,15],["cjk",15,16,15,15]],"message":null},
  {"text":"😀 👍🏽 你好","record":false,"nodes":[["items","infinite-repeat",0,10,0,9,"😀 👍🏽 你好"],["😀","literal",0,2,0,1,"😀"],["space","literal",2,3,2,2," "],["👍🏽","literal",3,7,3,6,"👍🏽"],["space","literal",7,8,7,7," "],["你好","literal",8,10,8,9,"你好"]],"error":null,"furthestError":["😀",8,9,8,8],"leafMatch":["你好",8,10],"errors":[],"message":null},
  {"text":"😀 👍🏽 你好","record":true,"nodes":[["items","infinite-repeat",0,10,0,9,"😀 👍🏽 你好"],["😀","literal",0,2,0,1,"😀"],["space","literal",2,3,2,2," "],["👍🏽","literal",3,7,3,6,"👍🏽"],["space","literal",7,8,7,7," "],["你好","literal",8,10,8,9,"你好"]],"error":null,"furthestError":["😀",8,9,8,8],"leafMatch":["你好",8,10],"errors":[["😀",3,5,3,4],["🎉",3,4,3,3],["😀",8,9,8,8],["🎉",8,9,8,8],["👍🏽",8,9,8,8],["emoji",8,9,8,8]],"message":null},
  {"text":"你好 世界 😀x","record":false,"nodes":[],"error":["items",8,10,8,9],"furthestError":["space",8,9,8,8],"leafMatch":["😀",6,8],"errors":[],"message":"Error at line 1, column 9. Hint: "},
  {"text":"你好 世界 😀x","record":true,"nodes":[],"error":["items",8,10,8,9],"furthestError":["space",8,9,8,8],"leafMatch":["😀",6,8],"errors":[["😀",0,1,0,0],["🎉",0,1,0,0],["👍🏽",0,1,0,0],["emoji",0,1,0,0],["😀",3,4,3,3],["🎉",3,4,3,3],["👍🏽",3,4,3,3],["emoji",3,4,3,3],["你好",3,4,3,3],["space",8,9,8,8],["items",8,10,8,9]],"message":"Error at line 1, column 9. Hint: "},
  {"text":"😀 你好 !","record":false,"nodes":[],"error":["items",6,8,6,7],"furthestError":["😀",6,7,6,6],"leafMatch":["space",5,6],"errors":[],"message":"Error at line 1, column 7. Hint: 😀,🎉,👍🏽,你好,世界"},
  {"text":"😀 你好 !","record":true,"nodes":[],"error":["items",6,8,6,7],"furthestError":["😀",6,7,6,6],"leafMatch":["space",5,6],"errors":[["😀",3,4,3,3],["🎉",3,4,3,3],["👍🏽",3,4,3,3],["emoji",3,4,3,3],["😀",6,7,6,6],["🎉",6,7,6,6],["👍🏽",6,7,6,6],["emoji",6,7,6,6],["你好",6,7,6,6],["世界",6,7,6,6],["cjk",6,7,6,6],["word",6,7,6,6],["item",6,7,6,6],["items",6,8,6,7]],"message":"Error at line 1, column 7. Hint: 😀,🎉,👍🏽,你好,世界"},
  {"text":"世界  hi","record":false,"nodes":[],"error":["items",3,7,3,6],"furthestError":["😀",3,4,3,3],"leafMatch":["space",2,3],"errors":[],"message":"Error at line 1, column 4. Hint: 😀,🎉,👍🏽,你好,世界"},
  {"text":"世界  hi","record":true,"nodes":[],"error":["items",3,7,3,6],"furthestError":["😀",3,4,3,3],"leafMatch":["space",2,3],"errors":[["😀",0,1,0,0],["🎉",0,1,0,0],["👍🏽",0,1,0,0],["emoji",0,1,0,0],["你好",0,1,0,0],["😀",3,4,3,3],["🎉",3,4,3,3],["👍🏽",3,4,3,3],["emoji",3,4,3,3],["你好",3,4,3,3],["世界",3,4,3,3],["cjk",3,4,3,3],["word",3,4,3,3],["item",3,4,3,3],["items",3,7,3,6]],"message":"Error at line 1, column 4. Hint: 😀,🎉,👍🏽,你好,世界"},
  {"text":"👍🏽 😀 你","record":false,"nodes":[],"error":["items",8,10,8,9],"furthestError":["😀",8,9,8,8],"leafMatch":["space",7,8],"errors":[],"message":"Error at line 1, column 9. Hint: 😀,🎉,👍🏽,你好,世界"},
  {"text":"👍🏽 😀 你","record":true,"nodes":[],"error":["items",8,10,8,9],"furthestError":["😀",8,9,8,8],"leafMatch":["space",7,8],"errors":[["😀",0,2,0,1],["🎉",0,1,0,0],["😀",8,9,8,8],["🎉",8,9,8,8],["👍🏽",8,9,8,8],["emoji",8,9,8,8],["你好",8,9,8,8],["世界",8,9,8,8],["cjk",8,9,8,8],["word",8,9,8,8],["item",8,9,8,8],["items",8,10,8,9]],"message":"Error at line 1, column 9. Hint: 😀,🎉,👍🏽,你好,世界"},
];

// Deviation (DESIGN §13): v12 compares a literal with the text by UTF-16
// unit, so "😀" against "👍🏽" (both start with the high surrogate D83D)
// gets one unit in, inside the emoji. cpat compares characters, and a
// position inside one doesn't exist, so it fails at the start. Only a
// recording parse lists that error; every other position is v12's.
function withCharacterLiterals(expected) {
  const errors = expected.errors.map(error => {
    const [name, start, end] = error;
    const insideAnEmoji = name === "😀" && end - start === 2 && expected.text.slice(start, start + 2) === "👍";
    return insideAnEmoji ? [name, start, start + 1, start, start] : error;
  });
  return { ...expected, errors };
}

function observe(text, record) {
  const { items } = patterns`${grammar}`;
  const { ast, cursor } = items.exec(text, record);
  const nodes = [];
  ast?.walkDown(n => {
    nodes.push([n.name, n.type, n.startIndex, n.endIndex, n.firstIndex, n.lastIndex, n.value]);
  });
  const err = e => (e == null ? null : [e.pattern.name, e.startIndex, e.endIndex, e.firstIndex, e.lastIndex]);
  return {
    text,
    record,
    nodes,
    error: err(cursor.error),
    furthestError: err(cursor.furthestError),
    leafMatch:
      cursor.leafMatch.node == null
        ? null
        : [cursor.leafMatch.pattern.name, cursor.leafMatch.node.startIndex, cursor.leafMatch.node.endIndex],
    errors: record ? cursor.errors.map(err) : [],
    message: ast == null ? generateErrorMessage(items, cursor) : null,
  };
}

for (const expected of EXPECTED) {
  test(`UTF-16 positions match v12: ${JSON.stringify(expected.text)}${expected.record ? " (recording)" : ""}`, () => {
    assert.deepEqual(observe(expected.text, expected.record), withCharacterLiterals(expected));
  });
}

test("the deviation adjusts only the two literal errors inside an emoji", () => {
  const changed = EXPECTED.filter(e => JSON.stringify(withCharacterLiterals(e)) !== JSON.stringify(e));
  assert.deepEqual(changed.map(e => e.text), ["😀 👍🏽 你好", "👍🏽 😀 你"]);
});
