//! The slice of JavaScript's value semantics that v12's attribute selectors
//! lean on. v12 compares a node's property with the selector's text using
//! JS operators, so `[endIndex>=35]` converts "35" to a number while
//! `[value>='35']` compares strings by UTF-16 code units.

use std::borrow::Cow;
use std::cmp::Ordering;

/// A node property's value, as JS sees it next to a string.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum JsValue {
    String(String),
    Number(f64),
    Boolean(bool),
    /// A node or an array of them: never `===` a string; compared and
    /// searched through its `toString()`, which is held here.
    Object(String),
}

impl JsValue {
    /// `value === text`. Only a string can be strictly equal to one.
    pub fn strict_equals(&self, text: &str) -> bool {
        matches!(self, JsValue::String(value) if value == text)
    }

    /// `String(value)`, for `startsWith`, `endsWith` and `includes`.
    pub fn to_js_string(&self) -> Cow<'_, str> {
        match self {
            JsValue::String(value) | JsValue::Object(value) => Cow::Borrowed(value),
            JsValue::Number(value) => Cow::Owned(number_to_string(*value)),
            JsValue::Boolean(value) => Cow::Borrowed(if *value { "true" } else { "false" }),
        }
    }

    /// How `value` compares with `text` under JS's relational operators:
    /// strings by UTF-16 code units, anything else as numbers. `None` when a
    /// side is `NaN`, where every relational operator is false.
    pub fn compare(&self, text: &str) -> Option<Ordering> {
        match self {
            JsValue::String(value) | JsValue::Object(value) => {
                Some(value.encode_utf16().cmp(text.encode_utf16()))
            }
            JsValue::Number(value) => value.partial_cmp(&to_number(text)),
            JsValue::Boolean(value) => f64::from(u8::from(*value)).partial_cmp(&to_number(text)),
        }
    }
}

/// `Number.prototype.toString` for the integers node properties hold.
fn number_to_string(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e21 {
        format!("{}", value as i64)
    } else {
        format!("{value}")
    }
}

/// JS's `Number(text)`: surrounding whitespace ignored, "" is 0, `0x`, `0o`
/// and `0b` prefixes, `Infinity`, decimals with an exponent; anything else
/// is `NaN`.
pub(crate) fn to_number(text: &str) -> f64 {
    let text = text.trim_matches(is_js_whitespace);
    if text.is_empty() {
        return 0.0;
    }
    if let Some(value) = radix_number(text) {
        return value;
    }
    let unsigned = text.strip_prefix(['+', '-']).unwrap_or(text);
    if unsigned == "Infinity" {
        return if text.starts_with('-') {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        };
    }
    // Rust's parser also takes "inf" and "NaN", which JS doesn't; within
    // these characters the two grammars agree.
    let decimal = text
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '.' | 'e' | 'E' | '+' | '-'));
    if decimal {
        text.parse().unwrap_or(f64::NAN)
    } else {
        f64::NAN
    }
}

/// `0x1F`, `0o17`, `0b11`: unsigned, as JS reads them.
fn radix_number(text: &str) -> Option<f64> {
    let bytes = text.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'0' {
        return None;
    }
    let radix = match bytes[1] {
        b'x' | b'X' => 16,
        b'o' | b'O' => 8,
        b'b' | b'B' => 2,
        _ => return None,
    };
    let digits = &text[2..];
    if digits.is_empty() {
        return Some(f64::NAN);
    }
    let mut value = 0.0;
    for c in digits.chars() {
        match c.to_digit(radix) {
            Some(digit) => value = value * f64::from(radix) + f64::from(digit),
            None => return Some(f64::NAN),
        }
    }
    Some(value)
}

/// JS's WhiteSpace and LineTerminator, which `Number()` trims. Unlike
/// `char::is_whitespace` it includes U+FEFF and leaves out U+0085.
fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\t' | '\n' | '\u{0B}' | '\u{0C}' | '\r' | ' ' | '\u{A0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200A}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202F}'
                | '\u{205F}'
                | '\u{3000}'
                | '\u{FEFF}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_reads_text_as_js_does() {
        assert_eq!(to_number("35"), 35.0);
        assert_eq!(to_number(" 35\n"), 35.0);
        assert_eq!(to_number(""), 0.0);
        assert_eq!(to_number("+.5"), 0.5);
        assert_eq!(to_number("1.e2"), 100.0);
        assert_eq!(to_number("-1e-1"), -0.1);
        assert_eq!(to_number("0x1F"), 31.0);
        assert_eq!(to_number("0b11"), 3.0);
        assert_eq!(to_number("-Infinity"), f64::NEG_INFINITY);
        assert_eq!(to_number("\u{FEFF}7"), 7.0);
        for nan in [
            "abc", "inf", "NaN", "infinity", "1e", ".", "-0x1", "0x", "1 2", "\u{85}1",
        ] {
            assert!(to_number(nan).is_nan(), "{nan:?}");
        }
    }

    #[test]
    fn strings_compare_by_utf16_code_units() {
        // U+FF5E is above U+1F600 by code point but below its surrogates.
        let wide = JsValue::String("\u{FF5E}".to_owned());
        assert_eq!(wide.compare("\u{1F600}"), Some(Ordering::Greater));
        assert_eq!(
            JsValue::String("10".into()).compare("9"),
            Some(Ordering::Less)
        );
    }

    #[test]
    fn numbers_compare_as_numbers_and_nan_compares_as_nothing() {
        assert_eq!(JsValue::Number(10.0).compare("9"), Some(Ordering::Greater));
        assert_eq!(JsValue::Number(10.0).compare("x"), None);
        assert_eq!(JsValue::Boolean(true).compare("1"), Some(Ordering::Equal));
    }

    #[test]
    fn only_strings_are_strictly_equal_to_text() {
        assert!(JsValue::String("5".into()).strict_equals("5"));
        assert!(!JsValue::Number(5.0).strict_equals("5"));
        assert!(!JsValue::Object("5".into()).strict_equals("5"));
        assert_eq!(JsValue::Number(5.0).to_js_string(), "5");
        assert_eq!(JsValue::Number(-1.0).to_js_string(), "-1");
    }
}
