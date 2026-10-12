use std::cell::Cell;

use crate::name::Name;

/// Identity of a pattern, shared by all of its clones.
///
/// Clones must keep their source's id: recursion detection asks "is a pattern
/// with this id already parsing at this position?", and every recursion level
/// is a fresh clone of the same definition.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PatternId(u32);

thread_local! {
    static NEXT_ID: Cell<u32> = const { Cell::new(0) };
}

impl PatternId {
    fn fresh() -> PatternId {
        NEXT_ID.with(|next| {
            let id = next.get();
            next.set(id + 1);
            PatternId(id)
        })
    }
}

/// What every pattern has: who it is and what it is called.
#[derive(Clone, Copy, Debug)]
pub struct Meta {
    pub id: PatternId,
    pub name: Name,
}

impl Meta {
    pub fn new(name: &str) -> Meta {
        Meta {
            id: PatternId::fresh(),
            name: Name::new(name),
        }
    }
}
