// Generated inputs and grammars for the micro workloads. Pure functions of
// their arguments and a fixed seed.
import { pick, random, repeatTo, utf8Bytes } from "./text.mjs";

// JavaScript's reserved words: a literal-only choice the LiteralSet mode
// indexes by first character. Longest first, so a word is never cut short by
// a shorter one it starts with ("in" / "instanceof") in either engine.
const KEYWORDS = [
  "abstract", "arguments", "await", "boolean", "break", "byte", "case", "catch", "char", "class",
  "const", "continue", "debugger", "default", "delete", "do", "double", "else", "enum", "eval",
  "export", "extends", "false", "final", "finally", "float", "for", "function", "goto", "if",
  "implements", "import", "in", "instanceof", "int", "interface", "let", "long", "native", "new",
  "null", "package", "private", "protected", "public", "return", "short", "static", "super",
  "switch", "synchronized", "this", "throw", "throws", "transient", "true", "try", "typeof",
  "var", "void", "volatile", "while", "with", "yield",
].sort((a, b) => b.length - a.length || (a < b ? -1 : 1));

export function keywordsGrammar() {
  const alternatives = KEYWORDS.map(k => `"${k}"`).join(" | ");
  return `# Every JS reserved word: a literal-only choice.\nkeyword = ${alternatives}\nspace = " "\nkeywords = (keyword, space)+\n`;
}

export function keywordsText(bytes) {
  const next = random(1);
  const words = Array.from({ length: 4096 }, () => pick(next, KEYWORDS));
  return repeatTo(words.join(" "), " ", bytes);
}

/** Code-shaped text for the regex tokenizer (identifiers, numbers,
 * strings, comments, punctuation, whitespace). */
export function tokensText(bytes) {
  const next = random(2);
  const idents = ["value", "index", "result", "_private", "$el", "makeThing", "x", "count2"];
  const lines = [];
  for (let i = 0; i < 400; i++) {
    const a = pick(next, idents);
    const b = pick(next, idents);
    const n = Math.floor(next() * 100000) / 100;
    const line = pick(next, [
      `let ${a} = ${b}(${n}, "str ${i} \\"q\\"");`,
      `if (${a} >= ${n} && !${b}) { return ${a}.${b}[${i}]; }`,
      `// comment ${i}: ${a} ${b}`,
      `${a} += ${b} * ${n} - ${i} % 7;`,
      `const ${a} = { ${b}: ${n}e3, other: "${a}" };`,
    ]);
    lines.push(line);
  }
  return repeatTo(lines.join("\n"), "\n", bytes);
}

/** A JSON document: an array of `count` generated records. */
export function jsonText(count, seed) {
  const next = random(seed);
  const words = ["alpha", "beta", "gamma", "delta", "with \\\"escapes\\\"", "tab\\tnewline\\n", "unicode \\u00e9"];
  const value = depth => {
    const roll = next();
    if (depth > 3 || roll < 0.45) {
      return pick(next, [
        () => `"${pick(next, words)}"`,
        () => String(Math.floor(next() * 1e6) / 100),
        () => String(-Math.floor(next() * 1000)),
        () => "true",
        () => "false",
        () => "null",
        () => `${Math.floor(next() * 100)}e-3`,
      ])();
    }
    if (roll < 0.7) {
      return `[${Array.from({ length: 1 + Math.floor(next() * 4) }, () => value(depth + 1)).join(", ")}]`;
    }
    const keys = Array.from({ length: 1 + Math.floor(next() * 4) }, (_, i) => `"${pick(next, words)}${i}": ${value(depth + 1)}`);
    return `{${keys.join(", ")}}`;
  };
  const record = i => `  {"id": ${i}, "name": "item ${i}", "tags": ["a", "b"], "data": ${value(0)}}`;
  return `[\n${Array.from({ length: count }, (_, i) => record(i)).join(",\n")}\n]`;
}

/** A JSON document of at least `bytes` UTF-8 bytes. */
export function jsonTextOfSize(bytes, seed) {
  let count = 64;
  let text = jsonText(count, seed);
  while (utf8Bytes(text) < bytes) {
    count = Math.ceil((count * bytes) / utf8Bytes(text)) + 1;
    text = jsonText(count, seed);
  }
  return text;
}

/** One long flat operator chain: `1+2*3-4/5+…` (no parentheses). */
export function arithFlat(bytes) {
  const next = random(3);
  const parts = ["1"];
  let size = 1;
  while (size < bytes) {
    const op = pick(next, ["+", "-", "*", "/"]);
    const n = String(1 + Math.floor(next() * 999));
    parts.push(op, n);
    size += 1 + n.length;
  }
  return parts.join("");
}

/** `depth` parentheses, each holding the one inside it and an operator:
 * `((((1+0)*1)+2)…)`. */
export function arithNested(depth) {
  let text = "1";
  for (let i = 0; i < depth; i++) text = `(${text}${i % 2 ? "*" : "+"}${i})`;
  return text;
}

// Words in several scripts, written so the grapheme path has work: accents
// precomposed and combining, emoji with skin tones and ZWJ sequences,
// regional-indicator flags, CJK, Cyrillic, Arabic, Devanagari.
const NON_ASCII_WORDS = [
  "café", "cafe\u0301", "naïve", "nai\u0308ve", "Ünïcödé", "straße", "ﬁnal", "İstanbul",
  "こんにちは", "世界", "東京都", "привет", "мир", "مرحبا", "नमस्ते", "한국어",
  "👍🏽", "👨‍👩‍👧", "🇯🇵", "❤️", "😀", "e\u0301\u0302",
];
const ASCII_WORDS = [
  "cafe", "cafeX", "naive", "naiveX", "Unicode", "strasse", "final", "Istanbul",
  "konnichiwa", "sekai", "tokyoto", "privet", "mir", "marhaba", "namaste", "hangug",
  "thumb", "family", "flagjp", "heart", "smile", "eee",
];
const SEPARATORS = [" ", ", ", ". ", "! ", " — ", "\n", " (", ") "];
const ASCII_SEPARATORS = [" ", ", ", ". ", "! ", " - ", "\n", " (", ") "];

function wordsText(words, separators, bytes, seed) {
  const next = random(seed);
  const parts = [];
  for (let i = 0; i < 3000; i++) parts.push(pick(next, words), pick(next, separators));
  parts.push("end");
  return repeatTo(parts.join(""), " ", bytes);
}

export const nonAsciiText = bytes => wordsText(NON_ASCII_WORDS, SEPARATORS, bytes, 4);

/** The same shape of text in ASCII only (same seed, same word and
 * separator choices), so the two words workloads differ by the grapheme
 * path and not by structure. */
export const asciiTwinText = bytes => wordsText(ASCII_WORDS, ASCII_SEPARATORS, bytes, 4);

/** A big generated grammar: `count` groups of literal, regex, sequence,
 * choice and repeat rules, and an expression over them. */
export function syntheticGrammar(count) {
  const lines = ["# Generated for the build benchmark.", "ws = /[ \\t]+/", 'comma = ","'];
  for (let i = 0; i < count; i++) {
    lines.push(
      `kw-${i} = "keyword${i}"`,
      `id-${i} = /x${i}[a-z_]+/`,
      `pair-${i} = kw-${i} + ws? + id-${i}`,
      `either-${i} = pair-${i} | kw-${i} | id-${i}`,
      `list-${i} = (either-${i}, comma trim)+`,
      `maybe-${i} = list-${i}? + ws? + !"stop${i}" + either-${i}`,
    );
  }
  const atoms = Array.from({ length: Math.min(count, 200) }, (_, i) => `either-${i}`).join(" | ");
  lines.push(
    'add = expr + ws? + "+" + ws? + expr',
    'mul = expr + ws? + "*" + ws? + expr',
    'neg = "-" + expr',
    'group = "(" + ws? + expr + ws? + ")"',
    `atom = ${atoms}`,
    "expr = neg | mul | add | group | atom",
    "program = (expr, ws)+",
  );
  return lines.join("\n") + "\n";
}
