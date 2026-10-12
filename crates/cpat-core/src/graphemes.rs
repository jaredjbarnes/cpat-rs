use unicode_segmentation::UnicodeSegmentation;

/// Where the grapheme boundaries of a text are.
///
/// Positions in cpat are byte offsets that always sit on a grapheme boundary,
/// so a match never ends halfway through `é` or an emoji. The question "is
/// this a boundary?" is answered once per text, at cursor creation:
///
/// - ASCII text (the common case for grammars and code) has a boundary at
///   every byte, so there is no table at all.
/// - Anything else gets one bit per byte, `len / 8` bytes in total.
pub(crate) enum Boundaries {
    Ascii,
    Table(Vec<u64>),
}

impl Boundaries {
    pub fn new(text: &str) -> Boundaries {
        if text.is_ascii() {
            return Boundaries::Ascii;
        }

        let mut bits = vec![0u64; text.len() / 64 + 1];
        let mut set = |i: usize| bits[i / 64] |= 1 << (i % 64);

        for (start, _) in text.grapheme_indices(true) {
            set(start);
        }
        set(text.len());

        Boundaries::Table(bits)
    }

    pub fn is_boundary(&self, i: usize) -> bool {
        match self {
            Boundaries::Ascii => true,
            Boundaries::Table(bits) => bits
                .get(i / 64)
                .is_some_and(|word| word & (1 << (i % 64)) != 0),
        }
    }

    /// The first boundary at or after `i`. `len` is always a boundary.
    pub fn snap_forward(&self, mut i: usize) -> usize {
        while !self.is_boundary(i) {
            i += 1;
        }
        i
    }

    /// The last boundary at or before `i`. `0` is always a boundary.
    pub fn snap_back(&self, mut i: usize) -> usize {
        while !self.is_boundary(i) {
            i -= 1;
        }
        i
    }
}
