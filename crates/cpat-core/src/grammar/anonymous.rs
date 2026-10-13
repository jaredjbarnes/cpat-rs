use std::cell::Cell;
use std::rc::Rc;

/// Numbers the anonymous patterns of ONE build: `anonymous-pattern-0`,
/// `-1`, … A top-level build starts its own count, so the same grammar
/// always gets the same names, whatever was built before it (v12
/// `AnonymousNames`). The grammars it builds on its behalf (imports, `with
/// params` blocks) share the count, so their names never collide with the
/// importer's: a collision would let a choice mistake an imported pattern's
/// child for a reference to itself.
#[derive(Debug, Default)]
pub(super) struct AnonymousNames {
    next: Rc<Cell<usize>>,
}

/// What every anonymous name starts with. A literal or regex built under
/// such a name is named by its content instead.
pub(super) const PREFIX: &str = "anonymous-pattern-";

impl AnonymousNames {
    pub(super) fn next(&self) -> String {
        let next = self.next.get();
        self.next.set(next + 1);
        format!("{PREFIX}{next}")
    }

    /// The same count, for a grammar built as part of this one.
    pub(super) fn shared(&self) -> AnonymousNames {
        AnonymousNames {
            next: Rc::clone(&self.next),
        }
    }
}

pub(super) fn is_anonymous(name: &str) -> bool {
    name.starts_with(PREFIX)
}
