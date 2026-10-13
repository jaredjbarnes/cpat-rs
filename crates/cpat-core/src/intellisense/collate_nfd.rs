//! Canonical decomposition (NFD) for the characters `collate` weighs
//! exactly: ICU normalizes before it weighs, so `"â" + cedilla` and
//! `"a" + cedilla + circumflex` are the same string to it
//! (`localeCompare` returns 0).
//!
//! Two steps, both from Unicode's normalization: a letter of U+00C0–U+017F
//! that decomposes becomes its base letter and its mark, and each run of
//! combining marks is put in canonical order: stably sorted by combining
//! class, so the cedilla and the ogonek (class 202, attached below) come
//! before the marks above (class 230), whatever order they were typed in.
//! Without the reordering the marks would weigh in typed order and
//! canonically equal strings would sort apart.
//!
//! Only the 13 marks below have a class here. Any other character is a
//! starter (class 0) and stops the reordering: right for every character of
//! the exact set (letters, ASCII, the no-break space, the ignorables), and
//! part of the approximation (DESIGN §17) for other marks.

use super::collate_latin::LATIN;

/// The combining marks the Latin table's letters decompose to, in the
/// order ICU's secondary weights put them, with their canonical combining
/// class.
pub(super) const MARKS: [(char, u8); 13] = [
    ('\u{301}', 230), // acute
    ('\u{300}', 230), // grave
    ('\u{306}', 230), // breve
    ('\u{302}', 230), // circumflex
    ('\u{30C}', 230), // caron
    ('\u{30A}', 230), // ring above
    ('\u{308}', 230), // diaeresis
    ('\u{30B}', 230), // double acute
    ('\u{303}', 230), // tilde
    ('\u{307}', 230), // dot above
    ('\u{327}', 202), // cedilla
    ('\u{328}', 202), // ogonek
    ('\u{304}', 230), // macron
];

/// `text` in NFD, as far as the exact set needs.
pub(super) fn decompose(text: &str) -> Vec<char> {
    let mut out = Vec::with_capacity(text.len());
    for c in text.chars() {
        match decomposition(c) {
            Some((base, mark)) => {
                out.push(base);
                push_ordered(mark, &mut out);
            }
            None => push_ordered(c, &mut out),
        }
    }
    out
}

/// A mark's place in `MARKS` (its secondary rank), if it is one.
pub(super) fn mark_index(c: char) -> Option<usize> {
    MARKS.iter().position(|&(mark, _)| mark == c)
}

/// The base letter and mark a Latin letter decomposes to. `collate_latin`
/// weighs such a letter `4 × rank` of its mark; any other weight is a
/// letter with no mark to decompose to (a stroke, `ŀ`, `ſ`).
fn decomposition(c: char) -> Option<(char, char)> {
    let index = LATIN.binary_search_by(|(listed, ..)| listed.cmp(&c)).ok()?;
    let (_, letter, weight, upper) = LATIN[index];
    if weight % 4 != 0 {
        return None;
    }
    let (mark, _) = *MARKS.get(usize::from(weight / 4).checked_sub(1)?)?;
    let base = if upper {
        letter.to_ascii_uppercase()
    } else {
        letter
    };
    Some((char::from(base), mark))
}

/// Appends `c`, moving a combining mark back past the marks of a higher
/// class before it (canonical ordering; a starter never moves and stops
/// marks from moving past it).
fn push_ordered(c: char, out: &mut Vec<char>) {
    let class = combining_class(c);
    let mut at = out.len();
    if class != 0 {
        while at > 0 && combining_class(out[at - 1]) > class {
            at -= 1;
        }
    }
    out.insert(at, c);
}

fn combining_class(c: char) -> u8 {
    MARKS
        .iter()
        .find(|&&(mark, _)| mark == c)
        .map_or(0, |&(_, class)| class)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_letter_decomposes_to_its_base_and_mark() {
        assert_eq!(decompose("Âç"), ['A', '\u{302}', 'c', '\u{327}']);
        // A stroke has no mark to decompose to.
        assert_eq!(decompose("ø"), ['ø']);
    }

    #[test]
    fn marks_below_move_before_marks_above_and_stop_at_a_starter() {
        let canonical = ['a', '\u{327}', '\u{302}'];
        assert_eq!(decompose("â\u{327}"), canonical);
        assert_eq!(decompose("a\u{302}\u{327}"), canonical);
        assert_eq!(decompose("a\u{327}\u{302}"), canonical);
        // Same class: typed order kept.
        assert_eq!(decompose("a\u{301}\u{300}"), ['a', '\u{301}', '\u{300}']);
        // A cedilla doesn't move past the letter it follows.
        assert_eq!(
            decompose("â b\u{327}"),
            ['a', '\u{302}', ' ', 'b', '\u{327}']
        );
    }
}
