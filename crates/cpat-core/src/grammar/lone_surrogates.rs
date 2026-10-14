//! Lone surrogate escapes in JSON text, made U+FFFD before `serde_json`
//! reads it.
//!
//! `JSON.parse` builds a JS string of UTF-16 units, so `"\ud800"` is a
//! string holding one unpaired surrogate. A Rust `String` can't hold one,
//! and `serde_json` refuses the text outright. A decorator argument follows
//! the rule cpat string literals already follow (`escapes.rs`): a `\u`
//! escape that is half of a surrogate pair with no partner reads as U+FFFD,
//! and two escapes that form a pair are still the one character they spell.

use std::borrow::Cow;

/// `json` with each lone surrogate escape written `\uFFFD`; borrowed when
/// there is none. Escapes are read left to right, each backslash consumed
/// once, so `\\ud800` (an escaped backslash, then `ud800`) is left alone.
/// Backslashes only occur inside strings in JSON, so no string tracking is
/// needed; text that isn't JSON is passed on for `serde_json` to refuse.
pub(crate) fn replace_lone_surrogates(json: &str) -> Cow<'_, str> {
    if !json.contains("\\u") {
        return Cow::Borrowed(json);
    }
    let mut out = String::with_capacity(json.len());
    let mut rest = json;
    let mut changed = false;

    while let Some(at) = rest.find('\\') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(unit) = unit_escape(rest) else {
            // `\` and the character it escapes, copied as they are.
            let escape_len = 1 + rest[1..].chars().next().map_or(0, char::len_utf8);
            out.push_str(&rest[..escape_len]);
            rest = &rest[escape_len..];
            continue;
        };
        let paired = is_high(unit) && unit_escape(&rest[6..]).is_some_and(is_low);
        if paired {
            out.push_str(&rest[..12]);
            rest = &rest[12..];
        } else if is_high(unit) || is_low(unit) {
            out.push_str("\\uFFFD");
            rest = &rest[6..];
            changed = true;
        } else {
            out.push_str(&rest[..6]);
            rest = &rest[6..];
        }
    }
    out.push_str(rest);

    if changed {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(json)
    }
}

/// The UTF-16 unit of the `\uHHHH` escape `text` starts with, if it does.
fn unit_escape(text: &str) -> Option<u16> {
    let hex = text.strip_prefix("\\u")?.get(..4)?;
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u16::from_str_radix(hex, 16).ok()
}

fn is_high(unit: u16) -> bool {
    (0xD800..=0xDBFF).contains(&unit)
}

fn is_low(unit: u16) -> bool {
    (0xDC00..=0xDFFF).contains(&unit)
}

#[cfg(test)]
mod tests {
    use super::replace_lone_surrogates;

    #[test]
    fn lone_surrogates_become_the_replacement_character() {
        let cases = [
            (r#""\ud800""#, r#""\uFFFD""#),
            (r#""\udfff""#, r#""\uFFFD""#),
            (r#"{"\ud800":1}"#, r#"{"\uFFFD":1}"#),
            (r#"["\udfff"]"#, r#"["\uFFFD"]"#),
            // A low half before a high half is two lone halves.
            (r#""\uDE00\uD83D""#, r#""\uFFFD\uFFFD""#),
            (r#""a\ud800b\u00e9""#, r#""a\uFFFDb\u00e9""#),
            (r#""\ud800\u0041""#, r#""\uFFFD\u0041""#),
        ];
        for (json, expected) in cases {
            assert_eq!(replace_lone_surrogates(json), expected, "{json}");
        }
    }

    #[test]
    fn pairs_other_escapes_and_escaped_backslashes_are_kept() {
        for json in [
            r#""\ud83d\ude00""#,
            r#""\uDBFF\uDFFF""#,
            r#""\\ud800""#,
            r#""\\\\ud800""#,
            r#""\"\n\u0000é😀""#,
            r#"{"a":[1,2.50]}"#,
        ] {
            assert_eq!(replace_lone_surrogates(json), json, "{json}");
        }
    }

    #[test]
    fn an_escaped_backslash_before_a_real_escape_still_lets_it_be_read() {
        assert_eq!(replace_lone_surrogates(r#""\\\ud800""#), r#""\\\uFFFD""#);
    }
}
