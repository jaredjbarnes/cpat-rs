//! `JSON.stringify` for a string, which v12's shadowed-literal message
//! prints the literals with.

use std::fmt::Write;

/// `text` as `JSON.stringify(text)` writes it: in double quotes, with `"`
/// and `\` escaped, the five short control escapes, and every other
/// control character below U+0020 as `\u00XX` (lowercase hex). Everything
/// else, including non-ASCII, is written as it is.
pub(crate) fn json_string(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('"');
    for c in text.chars() {
        match c {
            '"' => quoted.push_str("\\\""),
            '\\' => quoted.push_str("\\\\"),
            '\u{08}' => quoted.push_str("\\b"),
            '\u{0C}' => quoted.push_str("\\f"),
            '\n' => quoted.push_str("\\n"),
            '\r' => quoted.push_str("\\r"),
            '\t' => quoted.push_str("\\t"),
            c if c < ' ' => {
                let _ = write!(quoted, "\\u{:04x}", c as u32);
            }
            c => quoted.push(c),
        }
    }
    quoted.push('"');
    quoted
}

#[cfg(test)]
mod tests {
    use super::json_string;

    #[test]
    fn writes_what_json_stringify_writes() {
        // Each expected value is Node 24's `JSON.stringify` of the input.
        assert_eq!(json_string("=="), r#""==""#);
        assert_eq!(json_string("a\"b\\c"), r#""a\"b\\c""#);
        assert_eq!(
            json_string("\u{08}\u{0C}\n\r\t\u{0}\u{1F}\u{7F}"),
            "\"\\b\\f\\n\\r\\t\\u0000\\u001f\u{7F}\""
        );
        assert_eq!(json_string("é😀"), "\"é😀\"");
    }
}
