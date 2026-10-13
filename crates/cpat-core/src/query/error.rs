use std::fmt;

/// A selector that doesn't parse. v12 throws `Error` with this message; here
/// it is a value, so a bad selector from a user never panics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectorError {
    /// v12's message, word for word: `[Invalid Selector] Error at line 1,
    /// column 3. Hint: *,[,*,[`.
    pub message: String,
}

impl fmt::Display for SelectorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SelectorError {}
