use crate::pattern::Pattern;

/// How a `Repeat` repeats. The default is one or more with no divider.
#[derive(Clone, Debug)]
pub struct RepeatOptions {
    /// The fewest matches that count as success. Clamped up to 1: a repeat
    /// that may match nothing is written `Optional(Repeat)`, as v12 does.
    pub min: usize,
    /// The most matches, or `None` for no limit. A bound below `min` (or 0)
    /// becomes `min`.
    pub max: Option<usize>,
    /// Matched between repetitions, and kept in the tree.
    pub divider: Option<Pattern>,
    /// Drop a divider left dangling after the last repetition.
    pub trim_divider: bool,
}

impl Default for RepeatOptions {
    fn default() -> RepeatOptions {
        RepeatOptions {
            min: 1,
            max: None,
            divider: None,
            trim_divider: false,
        }
    }
}
