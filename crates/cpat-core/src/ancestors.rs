use crate::pattern_id::PatternId;

/// One composite that is parsing right now: who it is, where it started,
/// and which of its children it is on.
///
/// Patterns keep no parent pointers (DESIGN §2), so "which patterns enclose
/// this one, and where did they start?" is answered by the cursor instead.
/// It is the dynamic path from the root down to the pattern now parsing,
/// which is the same chain v12 walks through `parent`: a pattern parses only
/// while its parent does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    pub id: PatternId,
    /// Byte offset where this composite began parsing.
    pub start: usize,
    /// The index of the child this composite is parsing (or last parsed),
    /// in `Pattern::at_path`'s numbering. The frames' children, read from
    /// the bottom, are the path of the pattern parsing now: what a match or
    /// error is recorded under (`PatternPath`). Composites with one child
    /// leave it at 0.
    pub child: u32,
}

/// The stack of active composites, innermost on top.
///
/// Starts never decrease going up: a child begins at or after its parent's
/// start. So every frame that began at a given position sits in one run at
/// the top, and a scan from the top can stop at the first frame that began
/// earlier. That keeps the recursion guards proportional to how many
/// patterns started at this very spot, not to the depth of the parse.
#[derive(Clone, Debug, Default)]
pub(crate) struct Ancestors {
    frames: Vec<Frame>,
}

/// Which pattern a record is about, relative to the frames. Decides the
/// path it is recorded under; a path is only built when one is kept.
#[derive(Clone, Copy, Debug)]
pub(crate) enum At<'a> {
    /// A pattern with no frame of its own (a leaf, or a composite whose
    /// frame was refused for depth): the top frame's current child.
    Child,
    /// The composite that owns the top frame.
    Frame,
    /// A descendant of the composite that owns the top frame, these steps
    /// below it (a sequence recording its blocked lookahead child). With no
    /// frames at all, the steps from the root.
    Below(&'a [u32]),
}

impl Ancestors {
    #[inline]
    pub(crate) fn push(&mut self, id: PatternId, start: usize) {
        self.frames.push(Frame {
            id,
            start,
            child: 0,
        });
    }

    #[inline]
    pub(crate) fn pop(&mut self) {
        self.frames.pop();
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        self.frames.len()
    }

    /// The top composite moves on to child `index`. One store: every
    /// composite with more than one child pays it per child, so the path to
    /// whatever records next is always on hand without being built.
    #[inline]
    pub(crate) fn set_child(&mut self, index: u32) {
        if let Some(top) = self.frames.last_mut() {
            top.child = index;
        }
    }

    pub(crate) fn frames(&self) -> &[Frame] {
        &self.frames
    }

    /// Writes the path of the pattern `at` names into `out`.
    pub(crate) fn path_into(&self, at: At<'_>, out: &mut Vec<u32>) {
        out.clear();
        let above = |n: usize| self.frames[..n].iter().map(|frame| frame.child);
        let len = self.frames.len();
        match at {
            At::Child => out.extend(above(len)),
            At::Frame => out.extend(above(len.saturating_sub(1))),
            At::Below(steps) => {
                out.extend(above(len.saturating_sub(1)));
                out.extend_from_slice(steps);
            }
        }
    }

    /// How many active composites with `id` began at `pos`, counting no
    /// further than `limit` (callers only ever ask "any?" or "more than one?").
    ///
    /// The scan stops at the first frame that began before `pos`. A probe
    /// cursor (see `Cursor::probe`) inherits real starts while its own
    /// position is 0; nothing there is below 0, so it scans everything and
    /// still compares exactly, which is what v12 does with its parent chain.
    pub(crate) fn count_at(&self, id: PatternId, pos: usize, limit: usize) -> usize {
        let mut count = 0;
        for frame in self.frames.iter().rev() {
            if frame.start < pos {
                break;
            }
            if frame.start == pos && frame.id == id {
                count += 1;
                if count >= limit {
                    break;
                }
            }
        }
        count
    }
}
