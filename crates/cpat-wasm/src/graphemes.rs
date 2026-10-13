//! Where each character of a text starts, in UTF-16 offsets, for
//! `js/cursor.mjs`: v12's `Cursor` steps a grapheme at a time (`currentChar`,
//! `next`, `previous`, `getCharStartIndex`). The boundaries are the core's,
//! the ones a parse positions on, so the JS cursor and the parser never
//! disagree about where a character ends. *Rejected:* `Intl.Segmenter` in
//! JS (v12's source), whose Unicode version is the host's, not the parser's.

use wasm_bindgen::prelude::*;

use crate::utf16::Utf16Offsets;

/// The UTF-16 offset where each grapheme of `text` starts, in order.
#[wasm_bindgen(js_name = graphemeStarts)]
pub fn grapheme_starts(text: &str) -> Vec<u32> {
    let offsets = Utf16Offsets::new(text);
    cpat_core::grapheme_starts(text)
        .map(|start| offsets.of(start))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_in_utf16_units() {
        // "🔴" is two units, "é" one, "\r\n" one grapheme of two units.
        assert_eq!(grapheme_starts("🔴é\r\nx"), [0, 2, 3, 5]);
    }
}
