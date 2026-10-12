use crate::pattern::Pattern;
use crate::patterns::{Custom, Regex, TakeUntil};

/// Patterns whose suggested tokens come from outside (v12
/// `TokenizablePattern`): a regex has no literal text to offer, and take-until
/// and custom patterns take whatever text there is, so without help they
/// offer nothing to autocomplete. The grammar author (or a `@tokens`
/// decorator) hands them a list.
///
/// A capability of its own, not a method on every pattern: a `Literal`'s
/// tokens are its text and a `Sequence`'s are its children's, so setting
/// them would be a lie. Those types don't implement it, and
/// `Pattern::tokenizable_mut` says `None` for them.
pub trait Tokenizable {
    /// The tokens set, in order (empty until set).
    fn tokens(&self) -> &[String];

    /// Replaces the tokens. A clone keeps them.
    fn set_tokens(&mut self, tokens: Vec<String>);
}

impl Pattern {
    /// This pattern as a `Tokenizable`, when it is one: `Regex`,
    /// `TakeUntil` and `Custom`.
    pub fn tokenizable_mut(&mut self) -> Option<&mut dyn Tokenizable> {
        match self {
            Pattern::Regex(p) => Some(p),
            Pattern::TakeUntil(p) => Some(p),
            Pattern::Custom(p) => Some(p),
            Pattern::Literal(_)
            | Pattern::Sequence(_)
            | Pattern::Choice(_)
            | Pattern::Repeat(_)
            | Pattern::Optional(_)
            | Pattern::Not(_)
            | Pattern::Cut(_)
            | Pattern::Reference(_)
            | Pattern::Context(_)
            | Pattern::RightAssociated(_)
            | Pattern::Block(_) => None,
        }
    }
}

macro_rules! tokenizable {
    ($($kind:ident),*) => {$(
        impl Tokenizable for $kind {
            fn tokens(&self) -> &[String] {
                &self.tokens
            }

            fn set_tokens(&mut self, tokens: Vec<String>) {
                self.tokens = tokens;
            }
        }
    )*};
}

tokenizable!(Regex, TakeUntil, Custom);
