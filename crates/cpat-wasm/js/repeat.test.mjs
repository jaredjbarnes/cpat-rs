// clarity-pattern-parser v12's src/patterns/Repeat.test.ts, run against
// js/index.mjs with its assertions as written, and the accessor tests of
// FiniteRepeat.test.ts and InfiniteRepeat.test.ts (the min/max/clone ones)
// through `Repeat`, the class v12 exports: `{ max }` picks the bounded
// repeat, as v12's facade does. Where cpat differs, the assertion says so
// (DESIGN §22).
import { describe, test, expect } from "./testing/expect.mjs";
import { Cursor, Literal, Node, Regex, Repeat, Sequence, patterns } from "./index.mjs";

describe("Repeat", () => {
  test("Finite Repeat Without Min", () => {
    const number = new Regex("number", "\\d");
    const finiteRepeat = new Repeat("numbers", number, { max: 2 });

    let cursor = new Cursor("f");
    let result = finiteRepeat.parse(cursor);
    let expected = null;

    expect(result).toBeNull();
    expect(cursor.hasError).toBeTruthy();

    cursor = new Cursor("1");
    result = finiteRepeat.parse(cursor);
    expected = new Node("finite-repeat", "numbers", 0, 0, [new Node("regex", "number", 0, 0, [], "1")]);

    expect(result?.isEqual(expected)).toBe(true);
    expect(cursor.hasError).toBeFalsy();

    cursor = new Cursor("12");
    result = finiteRepeat.parse(cursor);
    expected = new Node("finite-repeat", "numbers", 0, 1, [
      new Node("regex", "number", 0, 0, [], "1"),
      new Node("regex", "number", 1, 1, [], "2"),
    ]);

    expect(result?.isEqual(expected)).toBe(true);
    expect(cursor.hasError).toBeFalsy();

    cursor = new Cursor("123");
    result = finiteRepeat.parse(cursor);
    expected = new Node("finite-repeat", "numbers", 0, 1, [
      new Node("regex", "number", 0, 0, [], "1"),
      new Node("regex", "number", 1, 1, [], "2"),
    ]);

    expect(result?.isEqual(expected)).toBe(true);
    expect(cursor.hasError).toBeFalsy();
    expect(cursor.index).toBe(1);
  });

  test("Finite Repeat With Min", () => {
    const number = new Regex("number", "\\d");
    const finiteRepeat = new Repeat("numbers", number, { max: 2, min: 2 });

    let cursor = new Cursor("f");
    let result = finiteRepeat.parse(cursor);
    let expected = null;

    expect(result).toBeNull();
    expect(cursor.hasError).toBeTruthy();

    cursor = new Cursor("1");
    result = finiteRepeat.parse(cursor);
    expected = null;

    expect(result).toBeNull();
    expect(cursor.hasError).toBeTruthy();

    cursor = new Cursor("12");
    result = finiteRepeat.parse(cursor);
    expected = new Node("finite-repeat", "numbers", 0, 1, [
      new Node("regex", "number", 0, 0, [], "1"),
      new Node("regex", "number", 1, 1, [], "2"),
    ]);

    expect(result?.isEqual(expected)).toBe(true);
    expect(cursor.hasError).toBeFalsy();

    cursor = new Cursor("123");
    result = finiteRepeat.parse(cursor);
    expected = new Node("finite-repeat", "numbers", 0, 1, [
      new Node("regex", "number", 0, 0, [], "1"),
      new Node("regex", "number", 1, 1, [], "2"),
    ]);

    expect(result?.isEqual(expected)).toBe(true);
    expect(cursor.hasError).toBeFalsy();
    expect(cursor.index).toBe(1);
  });

  test("Finite Repeat Get Tokens", () => {
    const number = new Literal("number", "1");
    const repeat = new Repeat("numbers", number);
    const numberClone = repeat.find(p => p.name === "number");

    let tokens = repeat.getTokens();
    let expected = ["1"];

    expect(tokens).toEqual(expected);

    tokens = repeat.getNextTokens();
    expected = [];

    expect(tokens).toEqual(expected);

    tokens = repeat.getTokensAfter(numberClone);
    expected = [];

    expect(tokens).toEqual(expected);
  });

  test("Get Tokens After", () => {
    const number = new Literal("number", "1");
    const repeat = new Repeat("numbers", number);
    const parent = new Sequence("parent", [repeat, new Literal("b", "B")]);
    const numberClone = parent.find(p => p.name === "number");
    const repeatClone = parent.children[0];

    const tokens = repeatClone.getTokensAfter(numberClone);
    const expected = ["B"];

    expect(tokens).toEqual(expected);
  });

  test("Get Next Tokens", () => {
    const number = new Literal("number", "1");
    const repeat = new Repeat("numbers", number);
    const parent = new Sequence("parent", [repeat, new Literal("b", "B")]);
    const repeatClone = parent.children[0];

    const tokens = repeatClone.getNextTokens();
    const expected = ["B"];

    expect(tokens).toEqual(expected);
  });

  test("Get Next Patterns", () => {
    const number = new Literal("number", "1");
    const repeat = new Repeat("numbers", number);
    const parent = new Sequence("parent", [repeat, new Literal("b", "B")]);
    const repeatClone = parent.children[0];
    const bClone = parent.find(p => p.name === "b");

    const patterns = repeatClone.getNextPatterns();
    const expected = [bClone];

    expect(patterns).toEqual(expected);
  });

  test("Repeat Get Patterns", () => {
    const number = new Literal("number", "1");
    const repeat = new Repeat("numbers", number);
    const numberClone = repeat.find(p => p.name === "number");

    let patterns = repeat.getPatterns();
    let expected = [numberClone];

    expect(patterns).toEqual(expected);

    patterns = repeat.getNextPatterns();
    expected = [];

    expect(patterns).toEqual(expected);

    patterns = repeat.getPatternsAfter(numberClone);
    expected = [];

    expect(patterns).toEqual(expected);
  });

  test("Repeat Properties", () => {
    const number = new Literal("number", "1");
    const repeat = new Repeat("numbers", number);

    expect(repeat.type).toBe("infinite-repeat");
    expect(repeat.name).toBe("numbers");
    expect(repeat.parent).toBeNull();
  });

  test("Repeat accessors return configured values", () => {
    const number = new Literal("number", "1");
    const repeat = new Repeat("numbers", number, { min: 2, max: 5 });

    expect(repeat.min).toBe(2);
    expect(repeat.max).toBe(5);
    expect(repeat.pattern.name).toBe("number");
    expect(repeat.options).toEqual({ min: 2, max: 5 });
  });

  test("Repeat default min and max", () => {
    const number = new Literal("number", "1");
    const repeat = new Repeat("numbers", number);

    expect(repeat.min).toBe(1);
    expect(repeat.max).toBe(Infinity);
  });

  test("test", () => {
    const number = new Literal("number", "1");
    const repeat = new Repeat("numbers", number);
    let result = repeat.test("1");
    let expected = true;

    expect(result).toBe(expected);

    result = repeat.test("g");
    expected = false;

    expect(result).toBe(expected);
  });

  test("Repeat Clone", () => {
    const number = new Literal("number", "1");
    const repeat = new Repeat("numbers", number);

    let repeatClone = repeat.clone();
    let expected = new Repeat("numbers", number);

    expect(repeatClone.isEqual(expected)).toBeTruthy();

    repeatClone = repeat.clone("new-name");
    expected = new Repeat("new-name", number);

    expect(repeatClone.isEqual(expected)).toBeTruthy();
  });
});

describe("FiniteRepeat accessors (through Repeat)", () => {
  test("Properties", () => {
    const numbers = new Repeat("numbers", new Regex("number", "\\d"), { min: 1, max: 3 });

    expect(numbers.type).toBe("finite-repeat");
    expect(numbers.name).toBe("numbers");
    expect(numbers.parent).toBeNull();
    // FiniteRepeat's slots, as written. v12's Repeat facade would say 1
    // (its inner repeat); cpat has no facade (DESIGN §22).
    expect(numbers.children.length).toBe(3);
    expect(numbers.min).toBe(1);
    expect(numbers.max).toBe(3);
  });

  test("Clone", () => {
    const numbers = new Repeat("numbers", new Regex("number", "\\d"), { min: 0, max: 3 });
    const clone = numbers.clone();

    expect(clone.type).toBe(numbers.type);
    expect(clone.name).toBe(numbers.name);
    expect(clone.parent).toBeNull();
    expect(clone.children.length).toBe(numbers.children.length);
    expect(clone.min).toBe(numbers.min);
    expect(clone.max).toBe(numbers.max);
  });

  test("Clone With Custom Overrides", () => {
    const numbers = new Repeat("numbers", new Regex("number", "\\d"), { min: 0, max: 3 });
    let clone = numbers.clone();
    let expected = new Repeat("numbers", new Regex("number", "\\d"), { min: 0, max: 3 });

    expect(clone.isEqual(expected)).toBeTruthy();

    clone = numbers.clone("cloned-numbers");
    expected = new Repeat("cloned-numbers", new Regex("number", "\\d"), { min: 0, max: 3 });

    expect(clone.isEqual(expected)).toBeTruthy();
  });
});

describe("InfiniteRepeat accessors (through Repeat)", () => {
  test("Properties", () => {
    const integer = new Repeat("integer", new Regex("digit", "\\d"));

    expect(integer.type).toBe("infinite-repeat");
    expect(integer.name).toBe("integer");
    expect(integer.min).toBe(1);
    expect(integer.parent).toBeNull();
    expect(integer.children[0].name).toBe("digit");
  });

  test("Clone With Custom Overrides", () => {
    const numbers = new Repeat("numbers", new Regex("number", "\\d"), {
      min: 3,
      divider: new Literal("divider", "divider"),
      trimDivider: true,
    });
    const clone = numbers.clone();
    const expected = new Repeat("numbers", new Regex("number", "\\d"), {
      min: 3,
      divider: new Literal("divider", "divider"),
      trimDivider: true,
    });

    expect(clone.isEqual(expected)).toBeTruthy();
  });
});

// Beyond v12's tests: what v12 answers for the same calls (printed by
// running v12's src with tsx).
describe("Repeat accessors, compared with v12", () => {
  test("a clone keeps min and max, bounded or not", () => {
    const bounded = new Repeat("r", new Literal("a", "a"), { min: 2, max: 5 }).clone();
    const unbounded = new Repeat("u", new Literal("a", "a"), { min: 0 }).clone("v");

    expect([bounded.min, bounded.max]).toEqual([2, 5]);
    expect([unbounded.min, unbounded.max]).toEqual([0, Infinity]);
  });

  test("options carry the divider and trimDivider, and the pattern is the one given", () => {
    const repeat = new Repeat("u", new Literal("a", "a"), {
      divider: new Literal(",", ","),
      trimDivider: true,
    });

    expect(repeat.options.min).toBe(1);
    expect(repeat.options.max).toBe(Infinity);
    expect(repeat.options.divider.name).toBe(",");
    expect(repeat.options.divider.isEqual(new Literal(",", ","))).toBe(true);
    expect(repeat.options.trimDivider).toBe(true);
    expect(repeat.pattern.isEqual(new Literal("a", "a"))).toBe(true);
    // v12's `pattern` is the pattern as given, which has no parent.
    expect(repeat.pattern.parent).toBeNull();
  });

  test("a bounded repeat of one still shows its divider", () => {
    const repeat = new Repeat("one", new Literal("a", "a"), { max: 1, divider: new Literal(",", ",") });

    expect(repeat.options.divider.name).toBe(",");
  });

  test("a repeat reached as a child answers for itself", () => {
    const parent = new Sequence("parent", [
      new Repeat("numbers", new Literal("number", "1"), { min: 2, max: 4 }),
      new Literal("b", "B"),
    ]);
    const child = parent.children[0];

    expect([child.min, child.max, child.pattern.name]).toEqual([2, 4, "number"]);
  });

  test("min and max are the bounds as given, as v12's facade reports them", () => {
    // The parse clamps (min at least 1, max at least min); the accessors
    // don't. A grammar's `*` asks its inner repeat for zero or more.
    const repeat = new Repeat("r", new Literal("a", "a"), { min: 0, max: 0 });
    expect([repeat.min, repeat.max, repeat.type]).toEqual([0, 0, "finite-repeat"]);
    expect(repeat.test("")).toBe(true);
    expect(repeat.exec("a").ast.value).toBe("a");

    const { star, upTo, exact, range, atLeast } = patterns`
      a = "a"
      star = (a)*
      up-to = (a){,3}
      exact = (a){2}
      range = (a){0,0}
      at-least = (a){0,}
    `;
    // Each rule is a context over its pattern, as in v12.
    const bounds = rule => {
      const pattern = rule.children[0];
      const repeat = pattern.type === "optional" ? pattern.children[0] : pattern;
      return [repeat.type, repeat.min, repeat.max];
    };
    expect([star, upTo, exact, range, atLeast].map(bounds)).toEqual([
      ["infinite-repeat", 0, Infinity],
      ["finite-repeat", 0, 3],
      ["finite-repeat", 2, 2],
      ["finite-repeat", 0, 0],
      ["infinite-repeat", 0, Infinity],
    ]);
  });
});
