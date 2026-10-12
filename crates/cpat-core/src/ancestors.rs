use crate::pattern_id::PatternId;

/// One composite that is parsing right now: who it is and where it started.
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
    /// Reserved for the intellisense milestones, which snapshot the path to
    /// the furthest leaf and need to know which child each frame was on.
    /// Always 0 until then: filling it would cost every composite a write
    /// per child for a feature nothing uses yet.
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

    pub(crate) fn frames(&self) -> &[Frame] {
        &self.frames
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
