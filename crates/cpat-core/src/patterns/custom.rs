use std::fmt;

use crate::cursor::Cursor;
use crate::pattern_id::Meta;
use crate::tree::NodeId;

/// A user-written pattern: the one variant parsed through dynamic dispatch.
///
/// The contract matches the built-ins: on success, return the node and leave
/// the cursor just past it; on failure, record an error and return `None`; to
/// succeed without consuming anything, return `None` with no error. A
/// zero-width node is taken as that zero-width success: it is dropped and
/// the cursor goes back to where the parse began.
pub trait CustomParse {
    fn parse(&mut self, meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId>;

    /// Patterns are cloned per recursion level, so a custom one must be too.
    fn clone_box(&self) -> Box<dyn CustomParse>;
}

impl Clone for Box<dyn CustomParse> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

#[derive(Clone)]
pub struct Custom {
    pub(crate) meta: Meta,
    parser: Box<dyn CustomParse>,
    // What intellisense suggests for it (`Tokenizable`): only its author
    // knows what text it takes.
    pub(crate) tokens: Vec<String>,
}

impl Custom {
    pub fn new(name: &str, parser: Box<dyn CustomParse>) -> Custom {
        Custom {
            meta: Meta::new(name),
            parser,
            tokens: Vec::new(),
        }
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();
        let node = self.parser.parse(self.meta, cursor)?;

        if cursor.node_end(node) > cursor.node_start(node) {
            return Some(node);
        }

        // A zero-width node breaks the contract every built-in keeps (no
        // built-in makes one), and a repeat that keeps it makes no progress
        // and would loop forever. Enforced here, at the one door user code
        // comes through, so the composites can rely on it: the node becomes
        // what the contract says it is, a success with nothing consumed.
        cursor.set_pos(first);
        None
    }
}

impl fmt::Debug for Custom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Custom")
            .field("meta", &self.meta)
            .finish_non_exhaustive()
    }
}
