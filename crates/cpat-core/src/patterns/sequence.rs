use crate::cursor::Cursor;
use crate::pattern::Pattern;
use crate::pattern_error::PatternError;
use crate::pattern_id::Meta;
use crate::tree::{NodeId, NodeKind};

/// Matches its children one after another.
#[derive(Clone, Debug)]
pub struct Sequence {
    pub(crate) meta: Meta,
    pub(crate) children: Vec<Pattern>,
    // Scratch space for one parse, reused across parses so steady-state
    // parsing doesn't allocate here. Safe because a pattern instance is never
    // re-entered: recursion always reaches a fresh clone.
    nodes: Vec<NodeId>,
}

impl Sequence {
    pub fn new(name: &str, children: Vec<Pattern>) -> Result<Sequence, PatternError> {
        if children.is_empty() {
            return Err(PatternError::NoChildren {
                name: name.to_string(),
                kind: "sequence",
            });
        }

        Ok(Sequence {
            meta: Meta::new(name),
            children,
            nodes: Vec::new(),
        })
    }

    pub fn children(&self) -> &[Pattern] {
        &self.children
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();
        self.nodes.clear();

        if !self.parse_children(cursor, first) {
            cursor.set_pos(first);
            return None;
        }

        // Every child was optional and none matched: no node, and no error.
        let Some(&last) = self.nodes.last() else {
            cursor.set_pos(first);
            return None;
        };

        let end = cursor.node_end(last);
        let node = cursor.branch(NodeKind::Sequence, self.meta.name, first, end, &self.nodes);
        cursor.record_match(self.meta, first, end);
        cursor.set_pos(cursor.snap_forward(end));
        Some(node)
    }

    fn parse_children(&mut self, cursor: &mut Cursor<'_>, first: usize) -> bool {
        let count = self.children.len();

        for i in 0..count {
            let before = cursor.pos();
            let node = self.children[i].parse(cursor);

            if cursor.has_error() {
                return false;
            }

            match node {
                Some(node) => self.nodes.push(node),
                // Zero-width success (an unmatched optional, a cut, a passing
                // lookahead): the next child starts where this one did.
                None => cursor.set_pos(before),
            }

            if i + 1 == count {
                if self.nodes.is_empty() && !self.children.iter().all(is_zero_width_ok) {
                    cursor.record_error(first, cursor.pos(), self.meta);
                    return false;
                }
                return true;
            }

            if node.is_some() && cursor.at_end() {
                return self.finish_at_end(cursor, first, i + 1);
            }
        }

        unreachable!("a sequence has at least one child")
    }

    /// The text ended with children still to go. An optional child is simply
    /// absent. A cut or a lookahead consumes nothing, so it still runs: a cut
    /// that was reached commits, and `!x` holds when nothing follows. Any other
    /// child means the input ended too early.
    fn finish_at_end(&mut self, cursor: &mut Cursor<'_>, first: usize, from: usize) -> bool {
        for child in &mut self.children[from..] {
            match child {
                Pattern::Optional(_) => {}
                Pattern::Cut(_) | Pattern::Not(_) => {
                    child.parse(cursor);
                    if cursor.has_error() {
                        return false;
                    }
                }
                _ => {
                    // `last` is the end itself: the input stopped short.
                    cursor.record_error(first, cursor.pos(), self.meta);
                    return false;
                }
            }
        }

        true
    }
}

/// Children allowed to produce no node at all without failing the sequence.
fn is_zero_width_ok(pattern: &Pattern) -> bool {
    matches!(pattern, Pattern::Optional(_) | Pattern::Cut(_))
}
