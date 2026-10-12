use crate::ancestors::At;
use crate::cursor::Cursor;
use crate::leaves::Parent;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
use crate::tree::{NodeId, NodeKind};

/// `?->| x`: takes text, one grapheme at a time, up to where `x` matches
/// (or to the end of the text). The terminator is never part of the node.
///
/// Taking nothing at all (the terminator matches right here) is a failure.
#[derive(Clone, Debug)]
pub struct TakeUntil {
    pub(crate) meta: Meta,
    pub(crate) terminator: Box<Pattern>,
    // What intellisense suggests for the free text (v12 `setTokens`). The
    // pattern can't know what text is welcome, so the grammar author says
    // (`Tokenizable`).
    pub(crate) tokens: Vec<String>,
}

impl TakeUntil {
    pub fn new(name: &str, terminator: Pattern) -> TakeUntil {
        TakeUntil {
            meta: Meta::new(name),
            terminator: Box::new(terminator.adopted()),
            tokens: Vec::new(),
        }
    }

    pub fn terminator(&self) -> &Pattern {
        &self.terminator
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();

        // Every probe of the terminator is thrown away (a match only marks
        // where to stop; it is never part of the node), so any cut a probe
        // fires is thrown away with it.
        let cut = cursor.cut();

        // v12 never probes past its last grapheme (`hasNext`), and neither
        // does this: the loop ends at the end of the text.
        while !cursor.at_end() {
            let on = cursor.pos();
            let terminator = &mut self.terminator;
            let terminated = cursor.detached(|c| terminator.parse(c)).is_some();
            cursor.set_cut(cut);
            cursor.resolve_error();
            cursor.set_pos(on);

            // A probe that met an unknown name or went too deep halted the
            // parse; scanning on would only fail every later probe on entry.
            if cursor.halted() {
                return None;
            }
            if terminated {
                break;
            }

            // Take the whole grapheme we are on.
            cursor.set_pos(cursor.snap_forward(on + 1));
        }

        let end = cursor.pos();
        if end == first {
            cursor.set_pos(first);
            cursor.record_error_at(first, first, self.meta, At::Frame);
            return None;
        }

        let node = cursor.leaf(NodeKind::TakeUntil, self.meta.name, first, end);
        cursor.record_match_at(self.meta, first, end, At::Frame, Parent::Above);
        Some(node)
    }
}
