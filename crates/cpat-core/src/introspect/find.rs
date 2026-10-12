//! v12 `find`: each pattern's own `find` method, and the `findPattern`
//! search most of them run.

use std::rc::Rc;

use crate::pattern::Pattern;
use crate::patterns::Repeat;

use super::Introspection;
use super::loc::{Loc, Node};

impl<'r> Introspection<'r> {
    /// `loc`'s own `find`. Leaves that are no search containers (literal,
    /// regex, take-until) and references answer nothing, not even
    /// themselves; a context searches from its child's own `find`; every
    /// other pattern runs `findPattern` on itself, so it can be the answer.
    pub(super) fn find_from(
        &self,
        loc: &Rc<Loc<'r>>,
        predicate: &dyn Fn(&Pattern) -> bool,
    ) -> Option<Rc<Loc<'r>>> {
        match loc.node {
            Node::Pattern(
                Pattern::Literal(_)
                | Pattern::Regex(_)
                | Pattern::TakeUntil(_)
                | Pattern::Reference(_),
            ) => None,
            Node::Pattern(Pattern::Context(_)) | Node::OwnContext { .. } => {
                let child = self.child(loc, 0)?;
                self.find_from(&child, predicate)
            }
            _ => self.find_pattern(loc, predicate),
        }
    }

    /// v12 `findPattern`: depth first, children before their parent, never
    /// into a reference (what it names is found by name, not contained).
    /// A choice's children are its classified children, as v12 searches
    /// them once built.
    fn find_pattern(
        &self,
        loc: &Rc<Loc<'r>>,
        predicate: &dyn Fn(&Pattern) -> bool,
    ) -> Option<Rc<Loc<'r>>> {
        if !matches!(loc.node, Node::Pattern(Pattern::Reference(_))) {
            for index in 0..searched_children(loc) {
                let Some(child) = self.child(loc, index) else {
                    break;
                };
                if let Some(found) = self.find_pattern(&child, predicate) {
                    return Some(found);
                }
            }
        }
        predicate(loc.pattern()).then(|| Rc::clone(loc))
    }
}

/// How many children a search looks at. Unbounded where the children run
/// out on their own (`child` says `None`), except a bounded repeat: its
/// slots repeat the child and divider (`{1,1000000}` has a million), and
/// a slot past the first child and divider holds the same patterns, so it
/// can't find anything they didn't.
fn searched_children(loc: &Loc<'_>) -> usize {
    match loc.node {
        Node::Pattern(Pattern::Repeat(repeat)) => bounded_slots(repeat).unwrap_or(usize::MAX),
        _ => usize::MAX,
    }
}

fn bounded_slots(repeat: &Repeat) -> Option<usize> {
    repeat.slots().map(|slots| slots.min(2))
}
