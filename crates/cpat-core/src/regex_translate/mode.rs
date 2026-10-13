use super::ast::Flags;

/// The flags in force at a point of the source. A source starts with none
/// (v12 compiles with `gu`); modifier groups (`(?i:…)`) change them.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Mode {
    pub ignore_case: bool,
    pub multiline: bool,
    pub dot_all: bool,
}

impl Mode {
    /// The mode inside a modifier group with these `flags`.
    pub fn with(self, flags: Flags) -> Mode {
        Mode {
            ignore_case: flags.ignore_case.unwrap_or(self.ignore_case),
            multiline: flags.multiline.unwrap_or(self.multiline),
            dot_all: flags.dot_all.unwrap_or(self.dot_all),
        }
    }
}
