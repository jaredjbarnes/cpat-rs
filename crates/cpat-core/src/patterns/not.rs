use crate::cursor::Cursor;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
use crate::tree::NodeId;

/// `!x`: succeeds, consuming nothing, when `x` does not match here.
#[derive(Clone, Debug)]
pub struct Not {
    pub(crate) meta: Meta,
    pub(crate) child: Box<Pattern>,
}

impl Not {
    pub fn new(name: &str, child: Pattern) -> Not {
        Not {
            meta: Meta::new(name),
            child: Box::new(child),
        }
    }

    pub fn child(&self) -> &Pattern {
        &self.child
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();
        let cut = cursor.cut();
        self.child.parse(cursor);
        let child_failed = cursor.has_error();

        // A lookahead never keeps its child's work, whichever way it went, so
        // a cut inside the child never commits.
        cursor.set_cut(cut);
        cursor.set_pos(first);
        cursor.resolve_error();

        if !child_failed {
            cursor.record_error(first, first, self.meta);
        }

        None
    }
}
