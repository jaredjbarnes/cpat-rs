use std::fmt;

/// A pattern that can't be built. Construction validates and fails early, so a
/// pattern that exists is a pattern that can run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatternError {
    EmptyLiteral {
        name: String,
    },
    EmptyRegex {
        name: String,
    },
    AnchoredRegex {
        name: String,
        source: String,
    },
    InvalidRegex {
        name: String,
        source: String,
        reason: String,
    },
    NoChildren {
        name: String,
        kind: &'static str,
    },
}

impl fmt::Display for PatternError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatternError::EmptyLiteral { name } => {
                write!(
                    f,
                    "literal `{name}` is empty; a literal must match at least one character"
                )
            }
            PatternError::EmptyRegex { name } => {
                write!(
                    f,
                    "regex `{name}` is empty; give it a pattern such as /[a-z]+/"
                )
            }
            PatternError::AnchoredRegex { name, source } => write!(
                f,
                "regex `{name}` (/{source}/) starts with `^` or ends with `$`; patterns already match at the cursor, so remove the anchor"
            ),
            PatternError::InvalidRegex {
                name,
                source,
                reason,
            } => {
                write!(f, "regex `{name}` (/{source}/) doesn't compile: {reason}")
            }
            PatternError::NoChildren { name, kind } => {
                write!(
                    f,
                    "{kind} `{name}` has no children; give it at least one pattern"
                )
            }
        }
    }
}

impl std::error::Error for PatternError {}
