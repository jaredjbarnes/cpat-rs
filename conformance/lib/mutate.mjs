// Mutations of a captured input, to reach errors and suggestions the tests
// themselves never asked about. Generated, not stored: the same corpus
// always expands to the same cases.

// Every token boundary gets a truncation. Deletions are capped per input,
// spread evenly over its boundaries: enough to break every kind of token in
// a typical input without letting the few long inputs dominate the run.
export const MAX_DELETIONS = 8;
// Inputs longer than this (a 500 KB stress line, a 3000-deep paren bomb)
// are compared as captured but not mutated.
export const MAX_MUTATED_LENGTH = 2000;

const kindOf = ch => (/[A-Za-z0-9_]/.test(ch) ? "word" : /\s/.test(ch) ? "space" : "other");

/** Code-point offsets (in UTF-16 units) where a token ends: between a word
 * and a space, and around every punctuation character. */
export function tokenBoundaries(text) {
  const boundaries = [];
  let offset = 0;
  let previous = null;
  for (const ch of text) {
    const kind = kindOf(ch);
    if (previous != null && (kind !== previous || kind === "other")) boundaries.push(offset);
    previous = kind;
    offset += ch.length;
  }
  return boundaries;
}

/** `count` items of `list`, evenly spaced, always including the last. */
export function spread(list, count) {
  if (list.length <= count) return list;
  const picked = [];
  for (let i = 1; i <= count; i++) picked.push(list[Math.ceil((i * list.length) / count) - 1]);
  return picked;
}

/** The input cut at token boundaries (the empty text first). */
export function truncations(text) {
  if (text.length > MAX_MUTATED_LENGTH) return [];
  return ["", ...tokenBoundaries(text).map(end => text.slice(0, end))];
}

/** The input with one character removed, at the first character and at
 * token boundaries. */
export function deletions(text) {
  if (text.length > MAX_MUTATED_LENGTH || text.length === 0) return [];
  return spread([0, ...tokenBoundaries(text)], MAX_DELETIONS).map(at => {
    const width = text.codePointAt(at) > 0xffff ? 2 : 1;
    return text.slice(0, at) + text.slice(at + width);
  });
}
