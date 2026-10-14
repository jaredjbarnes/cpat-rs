// clarity-pattern-parser v12's src/patterns/Cursor.test.ts, run against
// js/index.mjs with its assertions as written, then v12's answers on
// grapheme text and hand-recorded history, printed by running the same
// calls through v12's src with tsx and pinned here (DESIGN §22).
import { describe, test, expect } from "./testing/expect.mjs";
import { Cursor, Literal, Node, Regex, Sequence, patterns } from "./index.mjs";

const { name } = patterns`
    john = "John"
    jane = "Jane"
    first-names = john | jane

    doe = "Doe"
    smith = "Smith"
    last-names = doe | smith

    space = " "

    name = first-names + space + last-names
`;

describe("Cursor", () => {
  test("Empty Text", () => {
    const cursor = new Cursor("");

    cursor.next();
    expect(cursor.index).toBe(0);

    cursor.previous();
    expect(cursor.index).toBe(0);

    const text = cursor.substring(0, 1);
    expect(text).toBe("");
  });

  test("Move Cursor", () => {
    const cursor = new Cursor("Hello World!");
    cursor.moveTo(6);

    expect(cursor.currentChar).toBe("W");

    cursor.moveToFirstChar();
    cursor.previous();

    expect(cursor.isOnFirst).toBeTruthy();
    expect(cursor.currentChar).toBe("H");

    cursor.next();

    expect(cursor.currentChar).toBe("e");

    cursor.moveToLastChar();
    cursor.next();

    expect(cursor.isOnLast).toBeTruthy();
    expect(cursor.currentChar).toBe("!");

    cursor.previous();

    expect(cursor.currentChar).toBe("d");
  });

  test("Error Handling", () => {
    const pattern = new Literal("a", "A");
    const cursor = new Cursor("Hello World!");

    cursor.recordErrorAt(0, 0, pattern);

    expect(cursor.hasError).toBeTruthy();
    expect(cursor.error?.startIndex).toBe(0);
    expect(cursor.error?.lastIndex).toBe(0);
    expect(cursor.error?.pattern).toBe(pattern);

    cursor.resolveError();

    expect(cursor.hasError).toBeFalsy();
    expect(cursor.error).toBe(null);
    expect(cursor.furthestError?.startIndex).toBe(0);
    expect(cursor.furthestError?.lastIndex).toBe(0);
    expect(cursor.furthestError?.pattern).toBe(pattern);
  });

  test("Error Handling", () => {
    const pattern = new Literal("h", "H");
    const node = new Node("literal", "h", 0, 0, [], "H");

    const cursor = new Cursor("Hello World!");

    cursor.recordMatch(pattern, node);

    expect(cursor.leafMatch.node).toBe(node);
    expect(cursor.leafMatch.pattern).toBe(pattern);

    expect(cursor.rootMatch.node).toBe(node);
    expect(cursor.rootMatch.pattern).toBe(pattern);
  });

  test("Recording", () => {
    const cursor = new Cursor("Hello World!");
    cursor.startRecording();

    expect(cursor.isRecording).toBeTruthy();

    cursor.stopRecording();

    expect(cursor.isRecording).toBeFalsy();
  });

  test("Text Information", () => {
    const cursor = new Cursor("Hello World!");
    const hello = cursor.substring(0, 4);

    expect(hello).toBe("Hello");
    expect(cursor.length).toBe(12);
    expect(cursor.text).toBe("Hello World!");
    expect(cursor.index).toBe(0);
  });

  test("Records All matches", () => {
    const { ast, cursor } = name.exec("John Doe", true);
    const records = cursor.records;

    expect(ast?.toString()).toBe("John Doe");
    expect(records[0].ast?.toString()).toBe("John");
    expect(records[1].ast?.toString()).toBe(" ");
    expect(records[2].ast?.toString()).toBe("Doe");
    expect(records[3].ast?.toString()).toBe("John Doe");
  });

  test("Records Some Error Some Matches", () => {
    const { ast, cursor } = name.exec("John Smith", true);
    const records = cursor.records;

    expect(ast?.toString()).toBe("John Smith");
    expect(records[0].ast?.toString()).toBe("John");
    expect(records[1].ast?.toString()).toBe(" ");
    expect(records[2].error?.pattern.name).toBe("doe");
    expect(records[2].error?.startIndex).toBe(5);
    expect(records[2].error?.lastIndex).toBe(5);
    expect(records[3].ast?.toString()).toBe("Smith");
    expect(records[4].ast?.toString()).toBe("John Smith");
  });

  test("Records All Errors", () => {
    const { ast, cursor } = name.exec("Jack Smith", true);
    const records = cursor.records;

    expect(ast).toBeNull();
    expect(records[0].error).not.toBeNull();
    expect(records[0].pattern.name).toBe("john");
    expect(records[1].error).not.toBeNull();
    expect(records[1].pattern.name).toBe("jane");
    expect(records[2].error).not.toBeNull();
    expect(records[2].pattern.name).toBe("first-names");
  });

  test("Text with Emojis", () => {
    const cursor = new Cursor("🔴 World!");
    expect(cursor.currentChar).toBe("🔴");
    cursor.next();
    expect(cursor.currentChar).toBe(" ");
    expect(cursor.length).toBe(9);
  });

  test("allMatchedPatterns tracks patterns after recording", () => {
    const { cursor } = name.exec("John Doe", true);

    expect(cursor.allMatchedPatterns.length).toBeGreaterThan(0);
  });
});

// ─── v12's answers, pinned ───

// For each UTF-16 unit: [getCharStartIndex, getCharEndIndex,
// getCharLastIndex]; the characters `next` visits from the start and
// `previous` from `moveToLastChar`, as [index, currentChar]; and where
// `moveTo(unit)` lands. A skin-tone emoji is one character of four units,
// "\r\n" one of two.
const V12_TABLES = [
  {
    text: "a👍🏽b\r\nc",
    table: [[0, 1, 0], [1, 5, 4], [1, 5, 4], [1, 5, 4], [1, 5, 4], [5, 6, 5], [6, 8, 7], [6, 8, 7], [8, 9, 8]],
    forward: [[0, "a"], [1, "👍🏽"], [5, "b"], [6, "\r\n"], [8, "c"]],
    backward: [[8, "c"], [6, "\r\n"], [5, "b"], [1, "👍🏽"], [0, "a"]],
    moves: [0, 1, 1, 1, 1, 5, 6, 6, 8],
  },
  {
    // moveToLastChar sets the last unit, inside the emoji, as v12 does.
    text: "é🔴",
    table: [[0, 1, 0], [1, 3, 2], [1, 3, 2]],
    forward: [[0, "é"], [1, "🔴"]],
    backward: [[2, "🔴"], [0, "é"]],
    moves: [0, 1, 1],
  },
];

describe("Cursor characters, as v12 steps them", () => {
  for (const { text, table, forward, backward, moves } of V12_TABLES) {
    test(JSON.stringify(text), () => {
      const cursor = new Cursor(text);
      const units = [...Array(text.length).keys()];
      expect(units.map(i => [cursor.getCharStartIndex(i), cursor.getCharEndIndex(i), cursor.getCharLastIndex(i)])).toEqual(table);

      const seen = [];
      do {
        seen.push([cursor.index, cursor.currentChar]);
      } while (cursor.hasNext() && (cursor.next(), true));
      expect(seen).toEqual(forward);

      const back = [];
      cursor.moveToLastChar();
      back.push([cursor.index, cursor.currentChar]);
      while (cursor.hasPrevious()) {
        cursor.previous();
        back.push([cursor.index, cursor.currentChar]);
      }
      expect(back).toEqual(backward);

      expect(units.map(i => {
        const fresh = new Cursor(text);
        fresh.moveTo(i);
        return fresh.index;
      })).toEqual(moves);
    });
  }

  test("outside the text the char indices are v12's undefined and NaN", () => {
    const cursor = new Cursor("ab");
    expect(cursor.getCharStartIndex(2)).toBeUndefined();
    expect(Number.isNaN(cursor.getCharEndIndex(-1))).toBe(true);
    expect(Number.isNaN(cursor.getCharLastIndex(2))).toBe(true);
    expect(new Cursor("").currentChar).toBe("");
  });

  test("a direct parse leaves the cursor at the start of its last character", () => {
    // v12: 0 after "🔴" (lastIndex 1, inside the emoji), 2 after "🔴🔴".
    let cursor = new Cursor("🔴🔴x");
    new Literal("r", "🔴").parse(cursor);
    expect(cursor.index).toBe(0);

    cursor = new Cursor("🔴🔴x");
    new Regex("r", "🔴+").parse(cursor);
    expect(cursor.index).toBe(2);
    expect(cursor.hasError).toBe(false);
  });
});

describe("Cursor history by hand, as v12's CursorHistory keeps it", () => {
  const leaf = (name, first) => new Node("literal", name, first, first, [], "a");

  test("leaf matches: a sibling's tie is dropped, its parent's kept, a further match replaces both", () => {
    const seq = new Sequence("s", [new Literal("a", "a"), new Literal("b", "a")]);
    const [a, b] = seq.children;
    const cursor = new Cursor("aa");
    cursor.startRecording();

    cursor.recordMatch(a, leaf("a", 0));
    cursor.recordMatch(b, leaf("b", 0));
    expect(cursor.leafMatches.map(m => m.pattern.name)).toEqual(["a"]);

    cursor.recordMatch(seq, new Node("sequence", "s", 0, 0, [leaf("a", 0)]));
    expect(cursor.leafMatches.map(m => m.pattern.name)).toEqual(["s", "a"]);

    cursor.recordMatch(b, leaf("b", 1));
    expect(cursor.leafMatches.map(m => m.pattern.name)).toEqual(["b"]);
    expect([cursor.records.length, cursor.allMatchedNodes.length, cursor.rootMatch.pattern.name]).toEqual([4, 4, "b"]);
  });

  test("a summary error is current, but furthest only when nothing else failed", () => {
    const seq = new Sequence("s", [new Literal("a", "a"), new Literal("b", "b")]);
    const [a, b] = seq.children;

    let cursor = new Cursor("abc");
    cursor.recordErrorAt(0, 1, a);
    cursor.recordSummaryErrorAt(0, 2, seq);
    expect([cursor.error.lastIndex, cursor.furthestError.lastIndex, cursor.errors.length]).toEqual([2, 1, 0]);

    cursor = new Cursor("abc");
    cursor.recordSummaryErrorAt(0, 2, seq);
    expect([cursor.error.lastIndex, cursor.furthestError.lastIndex]).toEqual([2, 2]);

    cursor.startRecording();
    cursor.recordErrorAt(1, 1, b);
    cursor.recordErrorAt(0, 0, a);
    expect([cursor.error.lastIndex, cursor.furthestError.lastIndex, cursor.errors.length, cursor.records.length]).toEqual([0, 2, 2, 2]);
    expect(cursor.hasError).toBe(true);
  });

  test("recording can be switched after a parse reported", () => {
    const { cursor } = name.exec("John Doe");
    expect(cursor.isRecording).toBe(false);
    cursor.startRecording();
    expect(cursor.isRecording).toBe(true);
    cursor.recordErrorAt(0, 0, name);
    expect(cursor.errors.length).toBe(1);
  });

  test("choice scopes: a cut marks the innermost scope only", () => {
    const cursor = new Cursor("abc");
    const seen = [];
    cursor.pushChoiceScope();
    seen.push(cursor.getCut());
    cursor.markCut();
    seen.push(cursor.isCutInCurrentScope());
    cursor.pushChoiceScope();
    seen.push(cursor.getCut());
    cursor.popChoiceScope();
    seen.push(cursor.getCut());
    cursor.clearCut();
    seen.push(cursor.getCut());
    cursor.setCut(true);
    seen.push(cursor.getCut());
    cursor.popChoiceScope();
    // With no scope, marking and setting do nothing.
    cursor.markCut();
    seen.push(cursor.getCut());
    cursor.setCut(true);
    seen.push(cursor.isCutInCurrentScope());

    expect(seen).toEqual([false, true, false, true, false, true, false, false]);
  });
});

// v12's `CursorHistory` starts with one empty leaf match, which the first
// recorded match replaces; answers printed by running v12's src with tsx.
describe("Cursor's empty leaf match, compared with v12", () => {
  test("a fresh cursor has one, and it is the leaf match", () => {
    const cursor = new Cursor("abc");

    expect(cursor.leafMatches).toEqual([{ pattern: null, node: null }]);
    expect(cursor.leafMatch).toEqual({ pattern: null, node: null });
    expect(cursor.leafMatch).toBe(cursor.leafMatches[0]);
  });

  test("a parse that matched nothing leaves it", () => {
    const { cursor } = new Literal("a", "a").exec("b");

    expect(cursor.leafMatches).toHaveLength(1);
    expect(cursor.leafMatches[0].pattern).toBeNull();
    expect(cursor.leafMatches[0].node).toBeNull();
    expect(cursor.leafMatch).toBe(cursor.leafMatches[0]);
  });

  test("a parse that matched something replaces it, even when it fails later", () => {
    const sequence = new Sequence("s", [new Literal("a", "a"), new Literal("b", "b")]);
    const { cursor } = sequence.exec("ac");

    expect(cursor.leafMatches.map(m => [m.pattern.name, m.node.value])).toEqual([["a", "a"]]);
  });

  test("the first match recorded by hand replaces it", () => {
    const cursor = new Cursor("Hello");
    const pattern = new Literal("h", "H");
    const node = new Node("literal", "h", 0, 0, [], "H");

    cursor.recordMatch(pattern, node);

    expect(cursor.leafMatches).toHaveLength(1);
    expect(cursor.leafMatch.node).toBe(node);
  });
});
