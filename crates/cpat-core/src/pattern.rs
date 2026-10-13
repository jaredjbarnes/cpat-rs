use crate::ParseResult;
use crate::children::{self, Children};
use crate::cursor::Cursor;
use crate::exec::Record;
use crate::halt::Limits;
use crate::name::Name;
use crate::pattern_error::PatternError;
use crate::pattern_id::{Meta, PatternId};
use crate::pattern_path::PatternPath;
use crate::patterns::{
    Block, Choice, Context, Custom, Cut, Literal, Not, Optional, Reference, Regex, Repeat,
    RightAssociated, Sequence, TakeUntil,
};
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
    Repeat(Repeat),
    Optional(Optional),
    Not(Not),
    Cut(Cut),
    Reference(Reference),
    Context(Context),
    RightAssociated(RightAssociated),
    TakeUntil(TakeUntil),
    Block(Block),
    Custom(Custom),
}

impl Pattern {
    /// Tries to match at the cursor. On success the cursor sits just past the
    /// match. `None` with an error on the cursor is a failure; `None` without
    /// one is a zero-width success (an absent optional, a cut, a lookahead).
    ///
    /// Composites run inside an ancestor frame (DESIGN §2), pushed here so
    /// no composite can forget it; leaves have no children to tell.
    ///
    /// A halted cursor (an unknown name, input nested too deep) fails every
    /// pattern on entry, so the parse unwinds without trying anything else.
    #[inline]
    pub fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        if cursor.halted() {
            return None;
        }
        match self {
            Pattern::Literal(p) => p.parse(cursor),
            Pattern::Regex(p) => p.parse(cursor),
            Pattern::Cut(p) => p.parse(cursor),
            Pattern::Custom(p) => p.parse(cursor),
            // A marker, not a link in the pattern tree (v12 doesn't make it
            // its child's parent), so it pushes no frame.
            Pattern::RightAssociated(p) => p.parse(cursor),
            Pattern::Sequence(p) => cursor.framed(p.meta, |c| p.parse(c)),
            Pattern::Choice(p) => cursor.framed(p.meta, |c| p.parse(c)),
            Pattern::Repeat(p) => cursor.framed(p.meta, |c| p.parse(c)),
            Pattern::Optional(p) => cursor.framed(p.meta, |c| p.parse(c)),
            Pattern::Not(p) => cursor.framed(p.meta, |c| p.parse(c)),
            Pattern::Reference(p) => cursor.framed(p.meta, |c| p.parse(c)),
            Pattern::Context(p) => cursor.framed(p.meta, |c| p.parse(c)),
            Pattern::TakeUntil(p) => cursor.framed(p.meta, |c| p.parse(c)),
            Pattern::Block(p) => cursor.framed(p.meta, |c| p.parse(c)),
        }
    }

    /// Parses all of `text`. A match that stops short of the end is not a match.
    ///
    /// Before the first parse, references are readied: names are looked up in
    /// enclosing contexts, then in this pattern's own tree. A name found in
    /// neither comes back as `ParseResult::pattern_error` with no parse run.
    ///
    /// Input nested deeper than `Limits::default().max_depth` stops the parse
    /// with `ParseResult::too_deep` set; see `exec_with`.
    pub fn exec<'t>(&mut self, text: &'t str) -> ParseResult<'t> {
        crate::exec::exec(self, text, Limits::default(), Record::No)
    }

    /// `exec` with explicit limits.
    pub fn exec_with<'t>(&mut self, text: &'t str, limits: Limits) -> ParseResult<'t> {
        crate::exec::exec(self, text, limits, Record::No)
    }

    /// `exec` that keeps a history (v12 `exec(text, true)`): every error
    /// and match, in order, each with the path of the pattern instance that
    /// produced it (`ParseResult::errors`, `matches`, `records`,
    /// `root_match`). Replay a path with `at_path` on this pattern. A choice
    /// of literals runs its plain loop, so every literal's error is kept.
    pub fn exec_recording<'t>(&mut self, text: &'t str) -> ParseResult<'t> {
        crate::exec::exec(self, text, Limits::default(), Record::Yes)
    }

    /// `exec_recording` with explicit limits.
    pub fn exec_recording_with<'t>(&mut self, text: &'t str, limits: Limits) -> ParseResult<'t> {
        crate::exec::exec(self, text, limits, Record::Yes)
    }

    /// v12 `test(text)`: whether `exec` ends without an error
    /// (`ParseResult::passed`), so empty text passes.
    pub fn test(&mut self, text: &str) -> bool {
        self.exec(text).passed()
    }

    /// `test` on a recording parse (v12 `test(text, true)`): the same
    /// answer; only the work done to reach it differs.
    pub fn test_recording(&mut self, text: &str) -> bool {
        self.exec_recording(text).passed()
    }

    /// The pattern instance `path` leads to from this one, as a parse from
    /// here numbered it: the instance a recorded match or error came from.
    /// `None` when the path leads nowhere (a reference that hasn't
    /// resolved, a choice not yet classified, an index out of range).
    ///
    /// Child indices per kind (v12's `children` as the parse sees them):
    ///
    /// - `Sequence`: its children.
    /// - `Choice`: the classified list in declaration order, skipping
    ///   alternatives that are neither operand nor operator; an operator is
    ///   its sequence without the self-references.
    /// - `Repeat` without a bound: `[child, divider]`. With a bound, v12's
    ///   slots: child, divider, child, … (`max` children, and no divider
    ///   after the last with `trim_divider`); every slot is the one child or
    ///   divider here.
    /// - `Block`: `[open, content, close]`, without content when there is
    ///   none.
    /// - `Optional`, `Not`, `Context`, `TakeUntil`: child 0.
    /// - `Reference`: child 0 is its resolved instance.
    /// - `RightAssociated` is not a step: it is not a link in v12's pattern
    ///   tree, so a path goes straight through it to its child.
    pub fn at_path(&self, path: &PatternPath) -> Option<&Pattern> {
        path.steps()
            .iter()
            .try_fold(self.unmarked(), |pattern, &step| {
                let index = usize::try_from(step).ok()?;
                pattern.child_at(index).map(Pattern::unmarked)
            })
    }

    /// The child at `index` in `at_path`'s numbering.
    fn child_at(&self, index: usize) -> Option<&Pattern> {
        match self {
            Pattern::Literal(_) | Pattern::Regex(_) | Pattern::Cut(_) | Pattern::Custom(_) => None,
            Pattern::Sequence(p) => p.children.get(index),
            Pattern::Choice(p) => p.child_at(index),
            Pattern::Repeat(p) => p.child_at(index),
            Pattern::Block(p) => p.child_at(index),
            Pattern::Reference(p) => p.instance().filter(|_| index == 0),
            Pattern::Optional(p) => (index == 0).then_some(&*p.child),
            Pattern::Not(p) => (index == 0).then_some(&*p.child),
            Pattern::Context(p) => (index == 0).then_some(&*p.child),
            Pattern::TakeUntil(p) => (index == 0).then_some(&*p.terminator),
            Pattern::RightAssociated(p) => p.child.child_at(index),
        }
    }

    /// This pattern without `RightAssociated` markers around it.
    fn unmarked(&self) -> &Pattern {
        match self {
            Pattern::RightAssociated(marker) => marker.child.unmarked(),
            _ => self,
        }
    }

    /// Whether every reference a parse could reach resolves, with `exec`'s
    /// rules: enclosing contexts first, then this pattern's own tree. `exec`
    /// runs this itself (once per root); call it to fail early.
    pub fn check(&self) -> Result<(), PatternError> {
        crate::link::check(self)
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

    /// A clone under a new name (v12 `clone(name)`). It keeps the
    /// original's id: it is the same pattern, called something else, and
    /// the recursion guards recognise it.
    pub fn clone_named(&self, name: &str) -> Pattern {
        self.renamed_to(Name::new(name))
    }

    pub(crate) fn renamed_to(&self, name: Name) -> Pattern {
        let mut clone = self.clone();
        clone.rename(name);
        clone
    }

    /// v12 `clone(name)` without the clone. A context's child takes the
    /// name too, so a context resolved by a reference produces nodes named
    /// after the reference.
    fn rename(&mut self, name: Name) {
        // v12's `RightAssociated.clone(name)` ignores the name: the marker
        // stays nameless and its child keeps its own.
        if let Pattern::RightAssociated(_) = self {
            return;
        }
        self.meta_mut().name = name;
        match self {
            Pattern::Context(context) => context.child.rename(name),
            // Its wildcard content node is named after it.
            Pattern::Block(block) => block.renamed_content(),
            _ => {}
        }
    }

    /// This pattern as a composite's child. v12's composites clone their
    /// children (`clone()`, which keeps the name), and a cloned context
    /// passes its name down to its child; nothing else changes. Rust
    /// composites take their children over instead of cloning them, so they
    /// call this to come out the same.
    pub(crate) fn adopted(mut self) -> Pattern {
        if let Pattern::Context(context) = &mut self {
            let name = context.meta.name;
            context.child.rename(name);
        }
        self
    }

    /// The patterns this one contains, in order, as v12's `children` lists
    /// them for a search. A reference's target is not its child here: it is
    /// found by name, and searches never follow it.
    pub(crate) fn child_patterns(&self) -> children::Iter<'_> {
        match self {
            Pattern::Literal(_)
            | Pattern::Regex(_)
            | Pattern::Cut(_)
            | Pattern::Custom(_)
            | Pattern::Reference(_) => Children::Few([None, None, None]),
            Pattern::Sequence(p) => Children::Many(p.children.iter()),
            Pattern::Choice(p) => Children::Many(p.alternatives.iter()),
            Pattern::Repeat(p) => Children::Few([Some(p.child()), p.divider(), None]),
            Pattern::Optional(p) => Children::Few([Some(&*p.child), None, None]),
            Pattern::Not(p) => Children::Few([Some(&*p.child), None, None]),
            Pattern::Context(p) => Children::Few([Some(&*p.child), None, None]),
            Pattern::RightAssociated(p) => Children::Few([Some(&*p.child), None, None]),
            Pattern::TakeUntil(p) => Children::Few([Some(&*p.terminator), None, None]),
            Pattern::Block(p) => {
                Children::Few([Some(&*p.open), p.content.as_deref(), Some(&*p.close)])
            }
        }
    }

    pub(crate) fn child_patterns_mut(&mut self) -> children::IterMut<'_> {
        match self {
            Pattern::Literal(_)
            | Pattern::Regex(_)
            | Pattern::Cut(_)
            | Pattern::Custom(_)
            | Pattern::Reference(_) => Children::Few([None, None, None]),
            Pattern::Sequence(p) => Children::Many(p.children.iter_mut()),
            Pattern::Choice(p) => Children::Many(p.alternatives.iter_mut()),
            Pattern::Repeat(p) => {
                let (child, divider) = p.parts_mut();
                Children::Few([Some(child), divider, None])
            }
            Pattern::Optional(p) => Children::Few([Some(&mut *p.child), None, None]),
            Pattern::Not(p) => Children::Few([Some(&mut *p.child), None, None]),
            Pattern::Context(p) => Children::Few([Some(&mut *p.child), None, None]),
            Pattern::RightAssociated(p) => Children::Few([Some(&mut *p.child), None, None]),
            Pattern::TakeUntil(p) => Children::Few([Some(&mut *p.terminator), None, None]),
            Pattern::Block(p) => Children::Few([
                Some(&mut *p.open),
                p.content.as_deref_mut(),
                Some(&mut *p.close),
            ]),
        }
    }

    fn meta_ref(&self) -> &Meta {
        match self {
            Pattern::Literal(p) => &p.meta,
            Pattern::Regex(p) => &p.meta,
            Pattern::Sequence(p) => &p.meta,
            Pattern::Choice(p) => &p.meta,
            Pattern::Repeat(p) => &p.meta,
            Pattern::Optional(p) => &p.meta,
            Pattern::Not(p) => &p.meta,
            Pattern::Cut(p) => &p.meta,
            Pattern::Reference(p) => &p.meta,
            Pattern::Context(p) => &p.meta,
            Pattern::RightAssociated(p) => &p.meta,
            Pattern::TakeUntil(p) => &p.meta,
            Pattern::Block(p) => &p.meta,
            Pattern::Custom(p) => &p.meta,
        }
    }

    fn meta_mut(&mut self) -> &mut Meta {
        match self {
            Pattern::Literal(p) => &mut p.meta,
            Pattern::Regex(p) => &mut p.meta,
            Pattern::Sequence(p) => &mut p.meta,
            Pattern::Choice(p) => &mut p.meta,
            Pattern::Repeat(p) => &mut p.meta,
            Pattern::Optional(p) => &mut p.meta,
            Pattern::Not(p) => &mut p.meta,
            Pattern::Cut(p) => &mut p.meta,
            Pattern::Reference(p) => &mut p.meta,
            Pattern::Context(p) => &mut p.meta,
            Pattern::RightAssociated(p) => &mut p.meta,
            Pattern::TakeUntil(p) => &mut p.meta,
            Pattern::Block(p) => &mut p.meta,
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

into_pattern!(
    Literal,
    Regex,
    Sequence,
    Choice,
    Repeat,
    Optional,
    Not,
    Cut,
    Reference,
    Context,
    RightAssociated,
    TakeUntil,
    Block,
    Custom
);
