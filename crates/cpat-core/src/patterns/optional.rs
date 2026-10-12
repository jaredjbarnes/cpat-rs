use crate::cursor::Cursor;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
use crate::tree::NodeId;

/// `x?`: matches `x`, or nothing.
#[derive(Clone, Debug)]
pub struct Optional {
    pub(crate) meta: Meta,
    pub(crate) child: Box<Pattern>,
}

impl Optional {
    pub fn new(name: &str, child: Pattern) -> Optional {
        Optional {
            meta: Meta::new(name),
            child: Box::new(child.adopted()),
        }
    }

    pub fn child(&self) -> &Pattern {
        &self.child
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();
        let cut = cursor.cut();
        let node = self.child.parse(cursor);

        if cursor.has_error() {
            // The child's attempt is thrown away, and so is any commitment a
            // cut inside it made: only kept work commits.
            cursor.resolve_error();
            cursor.set_cut(cut);
            cursor.set_pos(first);
            return None;
        }

        node
    }
}
