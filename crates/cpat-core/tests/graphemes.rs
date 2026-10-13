//! `grapheme_starts`: where each character of a text starts, as the parser
//! positions on them, for hosts that step a character at a time (v12's
//! `Cursor.next`/`previous`, in cpat-wasm).

use cpat_core::grapheme_starts;

fn starts(text: &str) -> Vec<usize> {
    grapheme_starts(text).collect()
}

#[test]
fn ascii_starts_at_every_byte() {
    assert_eq!(starts("abc"), [0, 1, 2]);
}

#[test]
fn empty_text_has_no_characters() {
    assert_eq!(starts(""), Vec::<usize>::new());
}

#[test]
fn an_emoji_a_skin_tone_sequence_and_crlf_are_one_character_each() {
    // "🔴" is 4 bytes; "👍🏽" (thumbs up + skin tone) 8; "\r\n" one grapheme
    // (UAX #29 GB3), as v12's Intl.Segmenter has it.
    assert_eq!(starts("🔴 👍🏽\r\nx"), [0, 4, 5, 13, 15]);
}
