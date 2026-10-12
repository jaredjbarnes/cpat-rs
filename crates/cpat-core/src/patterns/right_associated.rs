use crate::cursor::Cursor;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
use crate::tree::NodeId;

/// Marks an infix alternative of a choice as right-associative:
/// `a ? b : c ? d : e` nests as `a ? b : (c ? d : e)`.
///
/// It is read once, when the choice classifies its alternatives; anywhere
/// else it is invisible and parses exactly as its child. It is a pattern of
/// its own rather than a flag on `Choice::new` so that it travels with the
/// alternative it marks (through a vector of alternatives, a reference, a
/// grammar), and so every `match` on `Pattern` has to say what it does.
///
/// Like v12's, it has no name (`""`) and no frame: it is not a link in the
/// pattern tree, so its child's recursion guards see past it.
#[derive(Clone, Debug)]
pub struct RightAssociated {
    pub(crate) meta: Meta,
    pub(crate) child: Box<Pattern>,
}

impl RightAssociated {
    pub fn new(child: Pattern) -> RightAssociated {
        RightAssociated {
            meta: Meta::new(""),
            child: Box::new(child.adopted()),
        }
    }

    pub fn child(&self) -> &Pattern {
        &self.child
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        self.child.parse(cursor)
    }
}
