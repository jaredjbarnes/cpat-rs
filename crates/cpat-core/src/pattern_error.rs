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
    /// The reference `name` looks up `target`, and neither an enclosing
    /// context nor the root pattern's tree defines it. Returned by
    /// `Pattern::check` and, without parsing, by `exec`.
    UnknownReference {
        name: String,
        target: String,
    },
    /// An alternative of the choice `choice` is a sequence of nothing but a
    /// reference to the choice itself: as an operator it would have no
    /// operator part, and as an operand it would only ever recurse.
    SelfOnlyAlternative {
        choice: String,
        alternative: String,
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
            PatternError::UnknownReference { name, target } => write!(
                f,
                "reference `{name}` looks up `{target}`, but no enclosing context and nothing in the pattern being run is named `{target}`; define it in a Context around the reference, or put it in the pattern's tree"
            ),
            PatternError::SelfOnlyAlternative {
                choice,
                alternative,
            } => write!(
                f,
                "alternative `{alternative}` of choice `{choice}` is a sequence of only `{choice}` itself, so it can neither be an operator nor match anything; remove it, or add the operator it should match around the self-reference"
            ),
        }
    }
}

impl std::error::Error for PatternError {}
