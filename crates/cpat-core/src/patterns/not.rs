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
            child: Box::new(child.adopted()),
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

    /// Whether the child matches the empty rest of the text: what `!x` sees
    /// at the very end. Probed on a throwaway empty cursor, as v12 does, so
    /// nothing the child records or commits reaches the real parse (a
    /// lookahead keeps none of its child's work anyway). The probe keeps the
    /// real cursor's scopes and ancestors, as v12's probe keeps its parent
    /// chain; this `Not` is on that chain, started where the probe starts.
    ///
    /// A probe that halted (too deep, an unknown name) halts the real parse;
    /// callers check `Cursor::halted` after asking.
    pub(crate) fn child_matches_empty(&mut self, cursor: &mut Cursor<'_>) -> bool {
        let mut probe = cursor.probe();
        let child = &mut self.child;
        probe.framed(self.meta, |probe| child.parse(probe));
        let matches = !probe.has_error();
        cursor.halt_from(probe, self.meta);
        matches
    }
}
