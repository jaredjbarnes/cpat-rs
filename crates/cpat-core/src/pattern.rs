use crate::ParseResult;
use crate::cursor::Cursor;
use crate::name::Name;
use crate::pattern_id::{Meta, PatternId};
use crate::patterns::{Choice, Custom, Cut, Literal, Not, Optional, Regex, Sequence};
use crate::tree::NodeId;

/// Every pattern cpat knows how to run.
///
/// Built-in patterns are dispatched with a `match`, which the compiler turns
/// into a jump table and can inline through. Only `Custom` goes through a
/// vtable, so the cost of extensibility lands on the patterns that use it.
///
/// A pattern owns its children and keeps per-parse scratch state on itself, so
/// `parse` takes `&mut self` and one instance is never parsed re-entrantly.
/// Recursion reaches a fresh clone, never the same instance.
#[derive(Clone, Debug)]
pub enum Pattern {
    Literal(Literal),
    Regex(Regex),
    Sequence(Sequence),
    Choice(Choice),
    Optional(Optional),
    Not(Not),
    Cut(Cut),
    Custom(Custom),
}

impl Pattern {
    /// Tries to match at the cursor. On success the cursor sits just past the
    /// match. `None` with an error on the cursor is a failure; `None` without
    /// one is a zero-width success (an absent optional, a cut, a lookahead).
    #[inline]
    pub fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        match self {
            Pattern::Literal(p) => p.parse(cursor),
            Pattern::Regex(p) => p.parse(cursor),
            Pattern::Sequence(p) => p.parse(cursor),
            Pattern::Choice(p) => p.parse(cursor),
            Pattern::Optional(p) => p.parse(cursor),
            Pattern::Not(p) => p.parse(cursor),
            Pattern::Cut(p) => p.parse(cursor),
            Pattern::Custom(p) => p.parse(cursor),
        }
    }

    /// Parses all of `text`. A match that stops short of the end is not a match.
    pub fn exec<'t>(&mut self, text: &'t str) -> ParseResult<'t> {
        crate::exec::exec(self, text)
    }

    pub fn test(&mut self, text: &str) -> bool {
        self.exec(text).is_match()
    }

    pub fn meta(&self) -> Meta {
        *self.meta_ref()
    }

    pub fn id(&self) -> PatternId {
        self.meta_ref().id
    }

    pub fn name(&self) -> &'static str {
        self.meta_ref().name.as_str()
    }

    /// A clone under a new name. It keeps the original's id: it is the same
    /// pattern, called something else.
    pub fn renamed(&self, name: &str) -> Pattern {
        let mut clone = self.clone();
        clone.meta_mut().name = Name::new(name);
        clone
    }

    fn meta_ref(&self) -> &Meta {
        match self {
            Pattern::Literal(p) => &p.meta,
            Pattern::Regex(p) => &p.meta,
            Pattern::Sequence(p) => &p.meta,
            Pattern::Choice(p) => &p.meta,
            Pattern::Optional(p) => &p.meta,
            Pattern::Not(p) => &p.meta,
            Pattern::Cut(p) => &p.meta,
            Pattern::Custom(p) => &p.meta,
        }
    }

    fn meta_mut(&mut self) -> &mut Meta {
        match self {
            Pattern::Literal(p) => &mut p.meta,
            Pattern::Regex(p) => &mut p.meta,
            Pattern::Sequence(p) => &mut p.meta,
            Pattern::Choice(p) => &mut p.meta,
            Pattern::Optional(p) => &mut p.meta,
            Pattern::Not(p) => &mut p.meta,
            Pattern::Cut(p) => &mut p.meta,
            Pattern::Custom(p) => &mut p.meta,
        }
    }
}

macro_rules! into_pattern {
    ($($kind:ident),*) => {$(
        impl From<$kind> for Pattern {
            fn from(pattern: $kind) -> Pattern {
                Pattern::$kind(pattern)
            }
        }
    )*};
}

into_pattern!(Literal, Regex, Sequence, Choice, Optional, Not, Cut, Custom);
