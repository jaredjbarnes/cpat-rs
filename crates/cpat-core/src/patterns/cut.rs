use crate::cursor::Cursor;
use crate::pattern_id::Meta;
use crate::tree::NodeId;

/// `~/`: commits the nearest enclosing choice to the alternative being tried.
///
/// Zero-width: it consumes nothing, makes no node and records no error. If the
/// committed alternative later fails, the choice fails instead of trying its
/// other alternatives, and keeps the deep error. If the alternative succeeds,
/// the cut changed nothing. With no enclosing choice it does nothing.
#[derive(Clone, Debug)]
pub struct Cut {
    pub(crate) meta: Meta,
}

impl Cut {
    pub fn new(name: &str) -> Cut {
        Cut {
            meta: Meta::new(name),
        }
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        cursor.mark_cut();
        None
    }
}

impl Default for Cut {
    fn default() -> Cut {
        Cut::new("cut")
    }
}
