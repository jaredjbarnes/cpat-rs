// Named checks an allowlist entry can require of a difference, for a
// difference whose values follow from the input by a rule too general for
// exact values or a regex. Each names the DESIGN decision it encodes.

const graphemes = new Intl.Segmenter("und", { granularity: "grapheme" });

/** Where the input's last grapheme starts, in UTF-16 units. */
function lastGraphemeStart(input) {
  let start = 0;
  for (const { index } of graphemes.segment(input)) start = index;
  return start;
}

/** A position past the end of the text, written as v12 writes it (one unit
 * past the start of the last grapheme, which is inside that grapheme when
 * it is wider than one unit) against cpat-rs's (the text's length).
 * DESIGN §11, "A `Sequence` that runs out of text records its error's
 * `last` at the end of the text". */
function pastTheEnd(input, v12, cpat) {
  const start = lastGraphemeStart(input);
  return input.length - start > 1 && v12 === start + 1 && cpat === input.length;
}

/** Whether UTF-16 unit `at` of `input` is the second half of a surrogate
 * pair. */
function insideASurrogatePair(input, at) {
  const unit = input.charCodeAt(at);
  return at > 0 && unit >= 0xdc00 && unit <= 0xdfff;
}

export const CHECKS = {
  /** An error triple whose `last` is that position (the start and the
   * pattern agree), or AutoComplete's `errorAtIndex`, which is read off it. */
  ranOutInsideAWideLastGrapheme(d) {
    const { expected: v12, actual: cpat, input } = d;
    if (Array.isArray(v12) && Array.isArray(cpat)) {
      return v12[0] === cpat[0] && v12[2] === cpat[2] && pastTheEnd(input, v12[1], cpat[1]);
    }
    return typeof v12 === "number" && pastTheEnd(input, v12, cpat);
  },

  /** An error triple whose start v12 puts between the halves of a surrogate
   * pair and cpat-rs at the pair's start, one unit earlier (the last index
   * and the pattern agree). DESIGN §5, the summary error's start after a
   * partial match is the matched value's length, a count of units. */
  startInsideASurrogatePair(d) {
    const { expected: v12, actual: cpat, input } = d;
    return (
      Array.isArray(v12) &&
      Array.isArray(cpat) &&
      v12[1] === cpat[1] &&
      v12[2] === cpat[2] &&
      cpat[0] === v12[0] - 1 &&
      insideASurrogatePair(input, v12[0])
    );
  },
};
