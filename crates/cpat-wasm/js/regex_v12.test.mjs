// JS regex semantics (the `u` flag) compared with clarity-pattern-parser v12:
// each row is [source, text, what v12's `new Regex("r", source).parse(new
// Cursor(text))?.value` gave], or "error" where v12's constructor threw.
// Recorded by running the same rows through v12's src under tsx (DESIGN §6).
import { test } from "node:test";
import assert from "node:assert/strict";
import { Cursor, Regex } from "./index.mjs";

const V12 = [
  ["a.b", "a\rb", null],
  ["a.b", "a\nb", null],
  ["a.b", "a\u2028b", null],
  ["a.b", "a\u2029b", null],
  ["a.b", "a b", "a b"],
  ["a.b", "a\u0085b", "a\u0085b"],
  ["a.+", "a\ud83d\ude00\u0301b\nc", "a\ud83d\ude00\u0301b"],
  ["a\\sb", "a\ufeffb", "a\ufeffb"],
  ["a\\sb", "a\u0085b", null],
  ["a\\Sb", "a\ufeffb", null],
  ["a\\Sb", "a\u0085b", "a\u0085b"],
  ["\\s+", "\t\n\u000b\f\r \u00a0\u1680\u2000\u200a\u2028\u2029\u202f\u205f\u3000\ufeffx", "\t\n\u000b\f\r \u00a0\u1680\u2000\u200a\u2028\u2029\u202f\u205f\u3000\ufeff"],
  ["[\\s]+", "\t\n\u000b\f\r \u00a0\u1680\u2000\u200a\u2028\u2029\u202f\u205f\u3000\ufeffx", "\t\n\u000b\f\r \u00a0\u1680\u2000\u200a\u2028\u2029\u202f\u205f\u3000\ufeff"],
  ["[^\\S]+", "\t\n\u000b\f\r \u00a0\u1680\u2000\u200a\u2028\u2029\u202f\u205f\u3000\ufeffx", "\t\n\u000b\f\r \u00a0\u1680\u2000\u200a\u2028\u2029\u202f\u205f\u3000\ufeff"],
  ["[^\\s]+", "\u180eab c", "\u180eab"],
  ["\\S+", "\u0085\u180e ", "\u0085\u180e"],
  ["a\\cJb", "a\nb", "a\nb"],
  ["\\cm", "\r", "\r"],
  ["[\\cI]", "\t", "\t"],
  ["\\v\\f\\0", "\u000b\f\u0000", "\u000b\f\u0000"],
  ["\\x41\\u0042\\u{43}", "ABC", "ABC"],
  ["\\uD83D\\uDE00", "\ud83d\ude00", "\ud83d\ude00"],
  ["[\\uD83D\\uDE00]+", "\ud83d\ude00\ud83d\ude00", "\ud83d\ude00\ud83d\ude00"],
  ["\\u{1F600}", "\ud83d\ude00", "\ud83d\ude00"],
  ["[\\b]", "\b", "\b"],
  ["\\/\\.\\*", "/.*", "/.*"],
  ["[\\-a]+", "-a-", "-a-"],
  ["\\d+", "12\u0663", "12"],
  ["\\w+", "abc_\u00e9", "abc_"],
  ["\\W", "\u00e9", "\u00e9"],
  ["\\bfoo\\b", "foo bar", "foo"],
  ["foo\\B", "foobar", "foo"],
  ["foo\\b", "foo\u00e9", "foo"],
  ["[^]+", "a\nb", "a\nb"],
  ["[]", "a", null],
  ["[^a]", "\n", "\n"],
  ["[a-c-e]+", "a-eb", "a-eb"],
  ["[\\d-]+", "1-2", "1-2"],
  ["[--a]+", "-.a", "-.a"],
  ["[a&&b]+", "a&b", "a&b"],
  ["[[]+", "[[", "[["],
  ["[~]", "~", "~"],
  ["[(){}|.*+?^$]+", "(){}|.*+?^$", "(){}|.*+?^$"],
  ["[^\\W\\d]+", "ab1", "ab"],
  ["over(?![a-z-])", "over-", null],
  ["over(?![a-z-])", "over ", "over"],
  ["a(?=b)", "ab", "a"],
  ["(?<=a)b", "ab", null],
  ["a(?<=a)b", "ab", "ab"],
  ["ab(?<!a)c", "abc", "abc"],
  ["x+(?<=xx)", "xxx", "xxx"],
  ["(?!if\\b)[a-z]+", "iffy", "iffy"],
  ["(a)\\1", "aa", "aa"],
  ["(a)?b\\1", "b", "b"],
  ["\\1(a)", "a", "a"],
  ["(?<q>['\"])x\\k<q>", "'x'", "'x'"],
  ["(?:(?<y>a)|(?<y>b))\\k<y>", "bb", "bb"],
  ["(a\\1)", "a", "a"],
  ["(?:([\"'])x\\1)+", "'x'\"x\"", "'x'\"x\""],
  ["(?:(a)b)+\\1", "ababa", "ababa"],
  ["\\p{L}+", "h\u00e9llo1", "h\u00e9llo"],
  ["\\p{Lu}", "a", null],
  ["\\P{Lu}+", "abC", "ab"],
  ["\\p{Script=Greek}+", "\u03b1\u03b2\u03b3a", "\u03b1\u03b2\u03b3"],
  ["\\p{sc=Latn}+", "abc", "abc"],
  ["\\p{scx=Hira}+", "\u3041\u30fc", "\u3041\u30fc"],
  ["[\\p{N}\\p{P}]+", "1,2a", "1,2"],
  ["\\p{Emoji_Presentation}", "\ud83d\ude00", "\ud83d\ude00"],
  ["\\p{Cs}", "a", null],
  ["\\P{Cs}", "a", "a"],
  ["\\p{gc=Letter}", "a", "a"],
  ["\\p{Any}", "a", "a"],
  ["[^\\p{ASCII}]", "\u00e9", "\u00e9"],
  ["(?i:k)", "\u212a", "\u212a"],
  ["(?i:\\w)", "\u017f", "\u017f"],
  ["(?i:\\W)", "\u017f", null],
  ["(?i:a\\b)", "a\u017f", null],
  ["(?m:a$)", "a\nb", "a"],
  ["(?m:a$)", "a\rb", "a"],
  ["(?m:a$\\n^b)", "a\nb", "a\nb"],
  ["(?s:a.b)", "a\nb", "a\nb"],
  ["(?i:[^a])", "A", null],
  ["(?i:\\p{Lu})", "a", "a"],
  // \P{…} under i: JS complements first, then folds (Rust's (?i) folds first).
  ["(?i:\\P{Lu})", "A", "A"],
  ["(?i:\\P{Ll})", "a", "a"],
  ["(?i:\\P{Lowercase})", "a", "a"],
  ["(?i:\\P{ASCII})", "k", "k"],
  ["(?i:\\P{Lt})", "\u01c6", "\u01c6"],
  ["(?i:[\\P{Lu}x])", "A", "A"],
  ["(?i:[^\\P{Lu}])", "a", null],
  ["(?i:[^\\P{Lu}])", "A", null],
  ["(?i:\\P{Any})", "a", null],
  ["(?i:[^\\P{Any}])", "a", "a"],
  ["(?i:(?-i:\\P{Lu}))", "A", null],
  ["(?i:\\P{Script=Greek})", "\u2126", null],
  ["(?i:\\P{Ll}+)", "aBc1", "aBc1"],
  ["(?i:[\\P{Lu}\\P{Ll}])", "A", "A"],
  ["(?i:\\P{Lu})", "\u212a", "\u212a"],
  ["(?i:a(?-i:b))", "AB", null],
  ["(?i:(a)\\1)", "aA", "aA"],
  ["(?i:[a-z]+)", "ABCd1", "ABCd"],
  ["[\\u{1F600}-\\u{1F64F}]+", "\ud83d\ude00\ud83d\ude4fx", "\ud83d\ude00\ud83d\ude4f"],
  ["[^\\uD800-\\uDFFF]", "a", "a"],
  ["[\\u0000-\\uFFFF]+", "a\u00e9\ud83d\ude00", "a\u00e9"],
  ["\\uD800", "a", null],
  ["\\u{61}+", "aab", "aa"],
  ["a{2,3}", "aaaa", "aaa"],
  ["a+?", "aaa", "a"],
  ["a{0}b", "b", "b"],
  ["a{99999999999999999999}", "a", null],
  ["[\\d-z]", "x", "error"],
  ["\\p{letter}", "a", "error"],
  ["\\pL", "a", "error"],
  ["a{,3}", "a", "error"],
  ["\\-", "-", "error"],
  ["\\x{41}", "A", "error"],
  ["(?=a)*", "a", "error"],
  ["\\c", "x", "error"],
  ["\\00", "x", "error"],
  ["(a)\\2", "aa", "error"],
  ["(?<a>x)(?<a>y)", "xy", "error"],
  ["\\a", "a", "error"],
  ["\\A", "a", "error"],
  ["\\z", "z", "error"],
  ["a*+", "a", "error"],
  ["(?P<a>x)", "x", "error"],
  ["[[:alpha:]]", "a", "error"],
  ["\\p{sc=latin}", "a", "error"],
  ["[\\w-a]", "a", "error"],
  ["\\e", "e", "error"],
  ["(?i)a", "a", "error"],
];

// JavaScript accepts these, but no Rust engine matches them as it does, so
// cpat-rs refuses to build them (DESIGN §6). v12's answers, for the record.
const REFUSED = [
  ["(?:(a)|b)+\\1", "abb", "abb"],
  ["(?:(a?))*b\\1", "aba", "aba"],
  ["(a)(?<=\\1)", "a", "a"],
  ["\\p{CWKCF}", "A", "A"],
  ["\\p{sc=Zzzz}", "\u0378", "\u0378"],
];

// Where cpat-rs follows the spec and V8 doesn't: [source, text, v12's
// answer, cpat-rs's answer] (DESIGN §6).
const DIFFERS = [
  ["(?i:\\u{212a})", "k", null, "k"],
  ["(?i:\\u{17f})", "s", null, "s"],
];

function run(source, text) {
  let regex;
  try {
    regex = new Regex("r", source);
  } catch {
    return "error";
  }
  return regex.parse(new Cursor(text))?.value ?? null;
}

test("regexes match what v12 matches, and fail to build where it fails", () => {
  const differences = V12.filter(([source, text, expected]) => run(source, text) !== expected)
    .map(([source, text, expected]) => ({ source, text, expected, got: run(source, text) }));
  assert.deepEqual(differences, []);
  assert.ok(V12.length >= 30);
});

test("where V8 strays from the spec, cpat-rs follows the spec", () => {
  for (const [source, text, v12, cpat] of DIFFERS) {
    assert.notEqual(v12, cpat);
    assert.equal(run(source, text), cpat, source);
  }
});

test("sources only JavaScript can match are refused when built", () => {
  for (const [source, text] of REFUSED) {
    assert.equal(run(source, text), "error", source);
  }
});
