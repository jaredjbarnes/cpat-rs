use std::fmt;

use crate::cursor::Cursor;
use crate::pattern_id::Meta;
use crate::tree::NodeId;

/// A user-written pattern: the one variant parsed through dynamic dispatch.
///
/// The contract matches the built-ins: on success, return the node and leave
/// the cursor just past it; on failure, record an error and return `None`; to
/// succeed without consuming anything, return `None` with no error.
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
}

impl Custom {
    pub fn new(name: &str, parser: Box<dyn CustomParse>) -> Custom {
        Custom {
            meta: Meta::new(name),
            parser,
        }
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        self.parser.parse(self.meta, cursor)
    }
}

impl fmt::Debug for Custom {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Custom")
            .field("meta", &self.meta)
            .finish_non_exhaustive()
    }
}
