//! Escapes in a cpat string literal, decoded in ONE left-to-right pass
//! (v12 `decodeEscapes`), so each backslash is consumed exactly once: `\\n`
//! is a backslash followed by `n`, never a backslash and a newline.

/// The text of a cpat string literal's body (without its quotes).
///
/// Supported: `\n \r \t \b \f \v \0`, `\xHH` and `\uHHHH`. Any other escaped
/// character (`\"`, `\\`, `\q`), and an `\x` or `\u` without enough hex
/// digits, means itself.
///
/// v12 builds a JS string of UTF-16 units, so two `\u` escapes can spell
/// one character as a surrogate pair (`😀` is `😀`); they are
/// combined here too. A surrogate with no partner has no place in a Rust
/// string and becomes U+FFFD, where v12 keeps the lone unit.
pub(crate) fn decode_escapes(body: &str) -> String {
    let mut decoded = Utf16Text::default();
    let mut chars = body.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\\' {
            decoded.push_char(c);
            continue;
        }
        let Some(&escaped) = chars.peek() else {
            // A backslash that ends the body means itself.
            decoded.push_char(c);
            continue;
        };
        chars.next();

        let digits = match escaped {
            'x' => 2,
            'u' => 4,
            _ => 0,
        };
        if digits > 0 {
            let hex: String = chars.clone().take(digits).collect();
            if hex.len() == digits && hex.chars().all(|d| d.is_ascii_hexdigit()) {
                let unit = u16::from_str_radix(&hex, 16).expect("checked: hex digits");
                decoded.push_unit(unit);
                chars.nth(digits - 1);
                continue;
            }
        }
        decoded.push_char(simple_escape(escaped).unwrap_or(escaped));
    }

    decoded.finish()
}

/// The one-letter escapes, as JS reads them.
fn simple_escape(letter: char) -> Option<char> {
    Some(match letter {
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        'b' => '\u{08}',
        'f' => '\u{0C}',
        'v' => '\u{0B}',
        '0' => '\0',
        _ => return None,
    })
}

/// Text built from characters and single UTF-16 units, so that a pair of
/// `\u` escapes forming a surrogate pair decodes to its one character.
#[derive(Default)]
struct Utf16Text {
    units: Vec<u16>,
}

impl Utf16Text {
    fn push_char(&mut self, c: char) {
        let mut buffer = [0; 2];
        self.units.extend_from_slice(c.encode_utf16(&mut buffer));
    }

    fn push_unit(&mut self, unit: u16) {
        self.units.push(unit);
    }

    fn finish(self) -> String {
        String::from_utf16_lossy(&self.units)
    }
}

#[cfg(test)]
mod tests {
    use super::decode_escapes;

    #[test]
    fn a_surrogate_pair_of_escapes_is_one_character() {
        assert_eq!(decode_escapes(r"😀"), "😀");
    }

    #[test]
    fn a_lone_surrogate_becomes_the_replacement_character() {
        assert_eq!(decode_escapes(r"a\uD83Db"), "a\u{FFFD}b");
    }

    #[test]
    fn a_trailing_backslash_means_itself() {
        assert_eq!(decode_escapes("a\\"), "a\\");
    }

    #[test]
    fn an_escaped_character_outside_the_bmp_means_itself() {
        assert_eq!(decode_escapes("\\😀x"), "😀x");
    }
}
