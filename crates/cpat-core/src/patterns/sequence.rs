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
            children: children.into_iter().map(Pattern::adopted).collect(),
            nodes: Vec::new(),
        })
    }

    pub fn children(&self) -> &[Pattern] {
        &self.children
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();
        self.nodes.clear();

        match self.parse_children(cursor, first) {
            Run::Matched => {}
            Run::ChildFailed => {
                cursor.set_pos(first);
                return None;
            }
            Run::FailedInPlace => {
                // v12 leaves its cursor where the failure was found; only
                // a repeat's error ever reads that spot (`last_index`).
                let stopped_at = cursor.last_index();
                cursor.set_pos(first);
                cursor.stopped_at(stopped_at);
                return None;
            }
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

    fn parse_children(&mut self, cursor: &mut Cursor<'_>, first: usize) -> Run {
        let count = self.children.len();

        for i in 0..count {
            let before = cursor.pos();
            let node = self.children[i].parse(cursor);

            if cursor.has_error() {
                return Run::ChildFailed;
            }

            if let Some(node) = node {
                self.nodes.push(node);
            }

            if i + 1 == count {
                // The last child is checked where it left the cursor, as in
                // v12: a child can return no node yet have moved (an
                // unbounded repeat over an optional does).
                if self.nodes.is_empty() && !self.children.iter().all(is_zero_width_ok) {
                    cursor.record_error(first, cursor.pos(), self.meta);
                    return Run::FailedInPlace;
                }
                return Run::Matched;
            }

            match node {
                Some(_) if cursor.at_end() => return self.finish_at_end(cursor, first, i + 1),
                Some(_) => {}
                // Zero-width success (an unmatched optional, a cut, a passing
                // lookahead): the next child starts where this one did.
                None => cursor.set_pos(before),
            }
        }

        unreachable!("a sequence has at least one child")
    }

    /// The text ended with children still to go. An optional child is simply
    /// absent. A cut consumes nothing, so it still runs: a cut that was
    /// reached commits. A lookahead `!x` holds unless `x` matches the empty
    /// rest of the text. Any other child means the input ended too early.
    fn finish_at_end(&mut self, cursor: &mut Cursor<'_>, first: usize, from: usize) -> Run {
        for child in &mut self.children[from..] {
            match child {
                Pattern::Optional(_) => {}
                Pattern::Cut(_) => {
                    child.parse(cursor);
                }
                Pattern::Not(not) => {
                    let matches = not.child_matches_empty(cursor);
                    if cursor.halted() {
                        return Run::ChildFailed;
                    }
                    if matches {
                        // The lookahead's own failure, just past the end.
                        let end = cursor.pos();
                        cursor.record_error(end, end, not.meta);
                        return Run::FailedInPlace;
                    }
                }
                _ => {
                    // `last` is the end itself: the input stopped short.
                    cursor.record_error(first, cursor.pos(), self.meta);
                    return Run::FailedInPlace;
                }
            }
        }

        Run::Matched
    }
}

/// How a sequence's children ended.
enum Run {
    Matched,
    /// A child failed; v12 rewinds to the sequence's start.
    ChildFailed,
    /// The sequence itself gave up (the text ended early, or nothing at all
    /// matched); v12 leaves its cursor where that was found.
    FailedInPlace,
}

/// Children allowed to produce no node at all without failing the sequence.
fn is_zero_width_ok(pattern: &Pattern) -> bool {
    matches!(pattern, Pattern::Optional(_) | Pattern::Cut(_))
}
