// The slice of jest's `expect` that v12's tests use, over node:test, so
// those tests port with their assertions as written.

import assert from "node:assert/strict";
export { describe, test } from "node:test";

// jest's `toEqual`: recursive equality of own enumerable properties,
// ignoring ones that are `undefined`, through cycles (a node's parent
// lists the node).
function equals(a, b, seen = new Map()) {
  if (Object.is(a, b)) {
    return true;
  }
  if (typeof a !== "object" || typeof b !== "object" || a === null || b === null) {
    return false;
  }
  if (Array.isArray(a) !== Array.isArray(b)) {
    return false;
  }
  let pairs = seen.get(a);
  if (pairs?.has(b)) {
    return true;
  }
  if (pairs == null) {
    pairs = new Set();
    seen.set(a, pairs);
  }
  pairs.add(b);

  if (Array.isArray(a)) {
    return a.length === b.length && a.every((item, i) => equals(item, b[i], seen));
  }
  const keys = o => Object.keys(o).filter(k => o[k] !== undefined);
  const ak = keys(a);
  const bk = keys(b);
  return ak.length === bk.length && ak.every(k => Object.hasOwn(b, k) && equals(a[k], b[k], seen));
}

function thrownMatches(thrown, expected) {
  if (expected === undefined) {
    return true;
  }
  const message = thrown instanceof Error ? thrown.message : String(thrown);
  if (typeof expected === "string") {
    return message.includes(expected);
  }
  if (expected instanceof RegExp) {
    return expected.test(message);
  }
  if (typeof expected === "function") {
    return thrown instanceof expected;
  }
  return equals(thrown, expected);
}

function matchers(actual, negate) {
  const check = (pass, message) => {
    if (pass === negate) {
      assert.fail(negate ? `not: ${message}` : message);
    }
  };
  const show = v => {
    try {
      return JSON.stringify(v);
    } catch {
      return String(v);
    }
  };

  return {
    toBe: expected => check(Object.is(actual, expected), `expected ${show(actual)} to be ${show(expected)}`),
    toEqual: expected => check(equals(actual, expected), `expected ${show(actual)} to equal ${show(expected)}`),
    toStrictEqual: expected => check(equals(actual, expected), `expected ${show(actual)} to equal ${show(expected)}`),
    toBeTruthy: () => check(Boolean(actual), `expected ${show(actual)} to be truthy`),
    toBeFalsy: () => check(!actual, `expected ${show(actual)} to be falsy`),
    toBeNull: () => check(actual === null, `expected ${show(actual)} to be null`),
    toBeUndefined: () => check(actual === undefined, `expected ${show(actual)} to be undefined`),
    toBeDefined: () => check(actual !== undefined, `expected a value, got undefined`),
    toBeInstanceOf: Class => check(actual instanceof Class, `expected an instance of ${Class.name}`),
    toContain: item =>
      check(
        typeof actual === "string" ? actual.includes(item) : actual.some(v => Object.is(v, item)),
        `expected ${show(actual)} to contain ${show(item)}`
      ),
    toHaveLength: length => check(actual.length === length, `expected length ${length}, got ${actual.length}`),
    toBeGreaterThan: n => check(actual > n, `expected ${actual} > ${n}`),
    toBeGreaterThanOrEqual: n => check(actual >= n, `expected ${actual} >= ${n}`),
    toBeLessThan: n => check(actual < n, `expected ${actual} < ${n}`),
    toBeLessThanOrEqual: n => check(actual <= n, `expected ${actual} <= ${n}`),
    toThrow: expected => {
      let thrown;
      let threw = false;
      try {
        actual();
      } catch (error) {
        threw = true;
        thrown = error;
      }
      const message = thrown instanceof Error ? thrown.message : String(thrown);
      check(threw && thrownMatches(thrown, expected), threw ? `threw "${message}", expected ${expected}` : "expected a throw");
    },
  };
}

export function expect(actual) {
  return {
    ...matchers(actual, false),
    not: matchers(actual, true),
    rejects: {
      async toThrow(expected) {
        let thrown;
        let threw = false;
        try {
          await actual;
        } catch (error) {
          threw = true;
          thrown = error;
        }
        const message = thrown instanceof Error ? thrown.message : String(thrown);
        assert.ok(
          threw && thrownMatches(thrown, expected),
          threw ? `rejected with "${message}", expected ${expected}` : "expected a rejection"
        );
      },
    },
  };
}
