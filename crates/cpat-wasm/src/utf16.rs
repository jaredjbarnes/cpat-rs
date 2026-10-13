/// Converts byte offsets (what cpat-core uses) into UTF-16 offsets (what JS
/// string indices are).
///
/// ASCII text maps every offset to itself, so it needs no table. Otherwise one
/// pass builds the map, and only when a result is actually handed to JS.
pub(crate) enum Utf16Offsets {
    Identity,
    Table(Vec<u32>),
}

impl Utf16Offsets {
    pub fn new(text: &str) -> Utf16Offsets {
        if text.is_ascii() {
            return Utf16Offsets::Identity;
        }

        let mut table = vec![0u32; text.len() + 1];
        let mut units = 0u32;
        for (byte, ch) in text.char_indices() {
            // Offsets inside a multi-byte character never escape cpat-core, but
            // fill them anyway so a lookup can't read garbage.
            for slot in &mut table[byte..byte + ch.len_utf8()] {
                *slot = units;
            }
            units += ch.len_utf16() as u32;
        }
        table[text.len()] = units;

        Utf16Offsets::Table(table)
    }

    pub fn of(&self, byte: usize) -> u32 {
        match self {
            Utf16Offsets::Identity => byte as u32,
            Utf16Offsets::Table(table) => table[byte.min(table.len() - 1)],
        }
    }
}

/// The byte offset of UTF-16 offset `unit` in `text`: JS hands positions in
/// as string indices. A unit inside a character (half of a surrogate pair)
/// is that character's start, and a unit past the end is the end.
pub(crate) fn byte_of(text: &str, unit: u32) -> usize {
    let mut units = 0u32;
    for (byte, ch) in text.char_indices() {
        let next = units + ch.len_utf16() as u32;
        if unit < next {
            return byte;
        }
        units = next;
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_identity() {
        let offsets = Utf16Offsets::new("abc");
        assert_eq!((offsets.of(0), offsets.of(3)), (0, 3));
    }

    #[test]
    fn counts_surrogate_pairs_as_two_units() {
        // "|" (1 byte, 1 unit), "🔴" (4 bytes, 2 units), "é" (2 bytes, 1 unit)
        let offsets = Utf16Offsets::new("|🔴é");
        assert_eq!(offsets.of(0), 0);
        assert_eq!(offsets.of(1), 1);
        assert_eq!(offsets.of(5), 3);
        assert_eq!(offsets.of(7), 4);
    }

    #[test]
    fn byte_of_inverts_of() {
        let text = "|🔴é";
        assert_eq!(byte_of(text, 0), 0);
        assert_eq!(byte_of(text, 1), 1);
        // Half of the surrogate pair is the emoji's start.
        assert_eq!(byte_of(text, 2), 1);
        assert_eq!(byte_of(text, 3), 5);
        assert_eq!(byte_of(text, 4), 7);
        assert_eq!(byte_of(text, 99), 7);
    }
}
