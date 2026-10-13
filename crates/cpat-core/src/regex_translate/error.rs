use std::fmt;

/// Why a JS regex source has no Rust translation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum TranslateError {
    /// JS's `u` flag rejects the source too, so v12 can't build it either.
    /// The text is V8's reason, so the two read alike.
    Syntax(&'static str),
    /// JS accepts the source, but neither Rust engine can match it the way
    /// JS does. Refused rather than matched differently (DESIGN §6).
    Unsupported(&'static str),
}

impl fmt::Display for TranslateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TranslateError::Syntax(reason) => {
                write!(f, "{reason} (JavaScript's `u` flag rejects it too)")
            }
            TranslateError::Unsupported(reason) => write!(
                f,
                "JavaScript accepts it, but cpat-rs can't match it the same way: {reason}"
            ),
        }
    }
}
