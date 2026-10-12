//! JS's default `localeCompare`, the order v12 sorts options in.
//!
//! v12 sorts with `a.text.localeCompare(b.text)`: ICU's collation for the
//! default locale (`en-US` in Node, which is CLDR root for these
//! characters). It is not code-point order: case is a tie-break, not a
//! separate block (`"a" < "A" < "b"`), punctuation sorts before digits and
//! digits before letters, in ICU's own punctuation order (`"_" < "-" < ","`),
//! accents outrank case (`"e" < "E" < "é"`), and control characters are
//! ignored entirely.
//!
//! This is the Unicode Collation Algorithm cut down to what suggestions
//! hold, with weights taken from Node (DESIGN §17): every ASCII character,
//! the Latin letters in U+00C0–U+017F (with a diacritic, a stroke, or
//! written as two letters: `æ` sorts as a special `ae`, `ß` as `ss`), and
//! the combining marks they decompose to, exactly; any other character
//! sorts after every letter, by code point. Three levels, each compared over the whole
//! string before the next: primary (the base character), secondary
//! (accents, as separate elements, so `"eá" < "éa"`), tertiary (case).
//!
//! *Rejected:* linking ICU (`icu_collator`): its data is megabytes in a
//! wasm module that is mostly regex tables already (DESIGN §6), for
//! characters no grammar in the corpus offers. *Rejected:* byte order: it
//! puts every capital before every lowercase letter and `"Z"` before `"a"`.

use std::cmp::Ordering;

use super::collate_latin::LATIN;

/// ICU's primary order for ASCII (and NEL), one entry per primary weight;
/// a letter's capital shares its weight. Generated from Node by sorting
/// every printable ASCII character with `localeCompare`.
/// The dotless `ı` is a letter of its own between `i` and `j`.
const ORDER: &str = "\t\n\u{b}\u{c}\r\u{85} _-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$\
                     0123456789abcdefghi\u{131}jklmnopqrstuvwxyz";

/// The combining marks the Latin table's letters decompose to, in the
/// order ICU's secondary weights put them (`collate_latin`).
const MARKS: [char; 13] = [
    '\u{301}', '\u{300}', '\u{306}', '\u{302}', '\u{30C}', '\u{30A}', '\u{308}', '\u{30B}',
    '\u{303}', '\u{307}', '\u{327}', '\u{328}', '\u{304}',
];

const COMMON: u16 = 1;
/// Tertiary weights, in DUCET's order: plain lower case, a letter written
/// as two (`æ`), upper case, an upper-case one written as two (`Æ`), and a
/// no-break space (a space at this level, after the plain one).
const LOWER: u8 = 2;
const COMPAT: u8 = 4;
const UPPER: u8 = 8;
const UPPER_COMPAT: u8 = 10;
const NO_BREAK: u8 = 27;
/// The secondary weight that sets a letter written as two (`æ`, `ß`) apart
/// from the two letters typed (`ae`, `ss`): above every diacritic, as in
/// DUCET.
const DIGRAPH: u16 = 200;

/// Letters ICU sorts as two: the letters, and whether upper case.
const DIGRAPHS: [(char, [char; 2], bool); 8] = [
    ('\u{C6}', ['a', 'e'], true),   // Æ
    ('\u{DF}', ['s', 's'], false),  // ß
    ('\u{E6}', ['a', 'e'], false),  // æ
    ('\u{132}', ['i', 'j'], true),  // Ĳ
    ('\u{133}', ['i', 'j'], false), // ĳ
    ('\u{152}', ['o', 'e'], true),  // Œ
    ('\u{153}', ['o', 'e'], false), // œ
    ('\u{1E9E}', ['s', 's'], true), // ẞ
];
/// Primaries for characters outside the tables start past every letter.
const UNLISTED: u32 = 1_000;

/// One collation element. A zero weight is ignorable at its level.
#[derive(Clone, Copy)]
struct Element {
    primary: u32,
    secondary: u16,
    tertiary: u8,
}

/// `a.localeCompare(b)` in JS's default locale, as an `Ordering`. Strings
/// that differ only in ignorable characters are `Equal`, as in JS, so a
/// stable sort keeps their order.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    let a = elements(a);
    let b = elements(b);
    level(&a, &b, |e| e.primary)
        .then_with(|| level(&a, &b, |e| u32::from(e.secondary)))
        .then_with(|| level(&a, &b, |e| u32::from(e.tertiary)))
}

/// One level: the non-zero weights in order, compared as sequences (a
/// shorter sequence that is a prefix sorts first).
fn level(a: &[Element], b: &[Element], weight: impl Fn(&Element) -> u32) -> Ordering {
    let a = a.iter().map(&weight).filter(|&w| w != 0);
    let b = b.iter().map(&weight).filter(|&w| w != 0);
    a.cmp(b)
}

fn elements(text: &str) -> Vec<Element> {
    let mut elements = Vec::with_capacity(text.len());
    for c in text.chars() {
        push_elements(c, &mut elements);
    }
    elements
}

fn push_elements(c: char, out: &mut Vec<Element>) {
    if is_ignorable(c) {
        return;
    }
    if let Some(primary) = ordered(c.to_ascii_lowercase()) {
        let tertiary = if c.is_ascii_uppercase() { UPPER } else { LOWER };
        out.push(base(primary, tertiary));
        return;
    }
    if c == '\u{A0}' {
        out.push(base(ordered(' ').expect("space is ordered"), NO_BREAK));
        return;
    }
    if let Some((_, letters, upper)) = DIGRAPHS.iter().find(|(listed, ..)| *listed == c) {
        let tertiary = if *upper { UPPER_COMPAT } else { COMPAT };
        let [first, second] = letters.map(|letter| ordered(letter).expect("an ASCII letter"));
        out.push(base(first, tertiary));
        out.push(Element {
            primary: 0,
            secondary: DIGRAPH,
            tertiary: COMPAT,
        });
        out.push(base(second, tertiary));
        return;
    }
    if let Some(weight) = mark_weight(c) {
        out.push(diacritic(weight));
        return;
    }
    if let Ok(index) = LATIN.binary_search_by(|(listed, ..)| listed.cmp(&c)) {
        let (_, letter, weight, upper) = LATIN[index];
        let primary = ordered(char::from(letter)).expect("a letter is ordered");
        out.push(base(primary, if upper { UPPER } else { LOWER }));
        out.push(diacritic(weight));
        return;
    }
    out.push(base(UNLISTED + u32::from(c), LOWER));
}

/// The primary weight of a character in `ORDER` (never zero).
fn ordered(c: char) -> Option<u32> {
    ORDER
        .chars()
        .position(|listed| listed == c)
        .map(|index| index as u32 + 1)
}

/// A combining mark's secondary weight, on `collate_latin`'s scale.
fn mark_weight(c: char) -> Option<u8> {
    MARKS
        .iter()
        .position(|&listed| listed == c)
        .map(|index| 4 * (index as u8 + 1))
}

fn base(primary: u32, tertiary: u8) -> Element {
    Element {
        primary,
        secondary: COMMON,
        tertiary,
    }
}

/// A diacritic: nothing at the primary level, its weight above `COMMON`
/// at the secondary.
fn diacritic(weight: u8) -> Element {
    Element {
        primary: 0,
        secondary: COMMON + u16::from(weight),
        tertiary: LOWER,
    }
}

/// ICU ignores control characters (other than the whitespace ones in
/// `ORDER`), the soft hyphen and the zero-width format characters.
fn is_ignorable(c: char) -> bool {
    match c {
        '\t'..='\r' | '\u{85}' => false,
        '\u{AD}' | '\u{200B}'..='\u{200F}' | '\u{2060}' | '\u{FEFF}' => true,
        _ => c.is_control(),
    }
}
