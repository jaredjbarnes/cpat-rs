//! A JS regex (`u` flag) as the parser reads it: only what the emitter and
//! the backreference check need, with escapes already decoded.

/// One node of the parsed source.
#[derive(Debug)]
pub(super) enum Node {
    /// `a|b`, two or more alternatives.
    Alternation(Vec<Node>),
    /// Terms one after another (possibly none: an empty alternative).
    Sequence(Vec<Node>),
    /// A code point. A lone surrogate is kept: no Rust text holds one, so it
    /// never matches.
    Char(u32),
    /// `.`
    Dot,
    /// `\d`, `\s`, `\p{…}` and their negations, outside a class.
    Set(Set),
    Class(Class),
    Assertion(Assertion),
    Group(Group),
    Look(Look),
    Repeat(Repeat),
    Backreference(Backreference),
}

/// A predefined set of characters.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Set {
    Digit {
        negated: bool,
    },
    Word {
        negated: bool,
    },
    Space {
        negated: bool,
    },
    /// `\p{…}`/`\P{…}`, the name as written (checked against JS's names).
    Property {
        negated: bool,
        name: String,
    },
    /// `\p{Surrogate}` (`Cs`): only lone surrogates, which no Rust text
    /// holds, and which Rust's tables leave out.
    Surrogates {
        negated: bool,
    },
}

#[derive(Debug)]
pub(super) struct Class {
    pub negated: bool,
    pub items: Vec<ClassItem>,
}

#[derive(Debug)]
pub(super) enum ClassItem {
    /// An inclusive code-point range (one character is `lo == hi`).
    Range(u32, u32),
    Set(Set),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Assertion {
    Start,
    End,
    WordBoundary,
    NotWordBoundary,
}

#[derive(Debug)]
pub(super) struct Group {
    pub kind: GroupKind,
    pub body: Box<Node>,
    /// Where the group closed, in the parser's event order: a
    /// backreference parsed later comes after the group in the source.
    pub end: usize,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum GroupKind {
    /// `(…)` or `(?<name>…)`: the capture's number, from 1.
    Capture(usize),
    NonCapture,
    /// `(?ims-ims:…)`.
    Modifiers(Flags),
}

/// A modifier group's changes: `Some(true)` adds the flag, `Some(false)`
/// removes it, `None` leaves it as the enclosing group has it.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Flags {
    pub ignore_case: Option<bool>,
    pub multiline: Option<bool>,
    pub dot_all: Option<bool>,
}

#[derive(Debug)]
pub(super) struct Look {
    pub behind: bool,
    pub negated: bool,
    pub body: Box<Node>,
}

#[derive(Debug)]
pub(super) struct Repeat {
    pub body: Box<Node>,
    pub min: u64,
    /// `None` is unbounded.
    pub max: Option<u64>,
    pub greedy: bool,
}

impl Repeat {
    /// Whether its body can run more than once, which is when JS's
    /// per-iteration reset of captures differs from Rust's engines.
    pub fn repeats(&self) -> bool {
        self.max.is_none_or(|max| max > 1)
    }
}

#[derive(Debug)]
pub(super) struct Backreference {
    /// The groups it may refer to: one for `\1`, every group of that name
    /// for `\k<name>` (JS lets alternatives reuse a name).
    pub groups: Vec<usize>,
    /// When it was parsed, in the parser's event order.
    pub at: usize,
}

impl Node {
    /// Whether it can match the empty string.
    pub fn nullable(&self) -> bool {
        match self {
            Node::Alternation(items) => items.iter().any(Node::nullable),
            Node::Sequence(items) => items.iter().all(Node::nullable),
            Node::Group(group) => group.body.nullable(),
            Node::Repeat(repeat) => repeat.min == 0 || repeat.body.nullable(),
            Node::Assertion(_) | Node::Look(_) | Node::Backreference(_) => true,
            Node::Char(_) | Node::Dot | Node::Set(_) | Node::Class(_) => false,
        }
    }
}
