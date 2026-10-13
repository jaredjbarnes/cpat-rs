//! Byte offsets into UTF-16 offsets, written here rather than borrowed from
//! cpat-wasm: the native engine must not share the code it is checking.

pub struct Utf16 {
    // `table[byte]` for every byte offset, or none for ASCII text.
    table: Option<Vec<u32>>,
}

impl Utf16 {
    pub fn new(text: &str) -> Utf16 {
        if text.is_ascii() {
            return Utf16 { table: None };
        }
        let mut table = vec![0; text.len() + 1];
        let mut units = 0u32;
        for (byte, ch) in text.char_indices() {
            table[byte..byte + ch.len_utf8()].fill(units);
            units += ch.len_utf16() as u32;
        }
        table[text.len()] = units;
        Utf16 { table: Some(table) }
    }

    pub fn of(&self, byte: usize) -> u32 {
        match &self.table {
            None => byte as u32,
            Some(table) => table[byte.min(table.len() - 1)],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Utf16;

    #[test]
    fn counts_a_surrogate_pair_as_two_units() {
        let offsets = Utf16::new("a😀é");
        assert_eq!([0, 1, 5, 7].map(|b| offsets.of(b)), [0, 1, 3, 4]);
    }
}
