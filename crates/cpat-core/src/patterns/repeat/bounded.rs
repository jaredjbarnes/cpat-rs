use crate::ancestors::At;
use crate::cursor::Cursor;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
use crate::pattern_path::step;
use crate::tree::{NodeId, NodeKind};

/// A repeat with an upper bound. It walks a fixed row of slots, child and
/// divider alternating (`max` children, and no divider after the last child
/// when `trim_divider` is set), and stops at the first failure or at the end
/// of the text. Mirrors v12's `FiniteRepeat`.
///
/// v12 clones the child into every slot. A slot's work is finished before
/// the next one starts, so one child and one divider serve every slot here:
/// a slot is an index, and `{1,1000}` doesn't build a thousand patterns.
#[derive(Clone, Debug)]
pub(super) struct Bounded {
    pub(super) child: Box<Pattern>,
    pub(super) divider: Option<Box<Pattern>>,
    pub(super) min: usize,
    pub(super) max: usize,
    pub(super) trim_divider: bool,
    slots: usize,
    // Scratch space for one parse, reused across parses.
    nodes: Vec<NodeId>,
    // Cut flag just before `nodes[i]` was parsed, so dropping that node can
    // undo its cut: only kept work commits.
    cuts_before: Vec<bool>,
}

impl Bounded {
    pub(super) fn new(
        child: Box<Pattern>,
        divider: Option<Box<Pattern>>,
        min: usize,
        max: usize,
        trim_divider: bool,
    ) -> Bounded {
        // Saturating: `max` can come from user input (JS passes up to
        // u32::MAX, and wasm's usize is 32 bits), and a wrapped count would
        // silently shrink the repeat. Past the text length the exact count
        // no longer matters: a consuming child runs out of text first, and
        // an idle lap ends the walk (see `fill_slots`).
        let slots = match (&divider, trim_divider) {
            (None, _) => max,
            (Some(_), false) => max.saturating_mul(2),
            // No divider slot after the last child. `max` is 0 only for
            // `{0}` / `{0,0}`, which has no slots at all.
            (Some(_), true) => max.saturating_mul(2).saturating_sub(1),
        };

        Bounded {
            child,
            divider,
            min,
            max,
            trim_divider,
            slots,
            nodes: Vec::new(),
            cuts_before: Vec::new(),
        }
    }

    pub(super) fn parse(&mut self, meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();
        self.nodes.clear();
        self.cuts_before.clear();

        let (match_count, mut kept_cut) = self.fill_slots(cursor);

        // v12 decides "ended on a divider" by parity: with a divider, an even
        // node count means the last node came from a divider slot.
        let ended_on_divider = self.divider.is_some() && self.nodes.len().is_multiple_of(2);
        if self.trim_divider && ended_on_divider {
            // Empty when nothing matched at all: with nothing to trim, this
            // falls through to the minimum check like the untrimmed repeat
            // does (v12 threw a TypeError here until its `a0ccba3`).
            if let Some(node) = self.nodes.pop() {
                kept_cut = self.cuts_before.pop().unwrap_or(kept_cut);
                cursor.set_pos(cursor.node_start(node));
            }
        }

        if match_count < self.min {
            let last = cursor.last_index();
            cursor.set_pos(first);
            cursor.record_error_at(first, last, meta, At::Frame);
            return None;
        }

        // From here on the repeat succeeds: a failed trailing slot (and a
        // trimmed divider) is dropped from the result, and so is any cut it
        // fired.
        cursor.set_cut(kept_cut);

        let (Some(&first_node), Some(&last_node)) = (self.nodes.first(), self.nodes.last()) else {
            // Only zero-width matches, or none with a minimum of 0: a
            // zero-width success, a failed slot dropped like any other, as
            // in v12 (which threw a TypeError when a later slot failed until
            // its `5801ae2`).
            cursor.resolve_error();
            cursor.set_pos(first);
            return None;
        };

        // Unlike the unbounded repeat (and as in v12), the node spans the
        // kept nodes, not from where the repeat started.
        let start = cursor.node_start(first_node);
        let end = cursor.node_end(last_node);
        cursor.resolve_error();
        let node = cursor.branch(NodeKind::FiniteRepeat, meta.name, start, end, &self.nodes);
        cursor.set_pos(cursor.snap_forward(end));
        Some(node)
    }

    /// Parses slot after slot. Returns how many child slots matched (a
    /// zero-width match counts, as in v12) and the cut flag as of the last
    /// slot that succeeded.
    fn fill_slots(&mut self, cursor: &mut Cursor<'_>) -> (usize, bool) {
        let mut match_count = 0;
        let mut kept_cut = cursor.cut();
        // Whether the previous slot was a child slot that matched nothing.
        let mut child_was_idle = false;

        for slot in 0..self.slots {
            let is_child_slot = self.divider.is_none() || slot % 2 == 0;
            let pattern = match (&mut self.divider, is_child_slot) {
                (Some(divider), false) => divider,
                _ => &mut self.child,
            };

            let running = cursor.pos();
            let cut_before = cursor.cut();
            // Each slot is its own child in v12, which clones the child
            // and divider into every slot.
            cursor.set_child(step(slot));
            let node = pattern.parse(cursor);

            if cursor.has_error() {
                break;
            }

            kept_cut = cursor.cut();
            if is_child_slot {
                match_count += 1;
            }

            match node {
                None => {
                    cursor.set_pos(running);
                    // A lap (child, then divider if any) that consumed
                    // nothing leaves everything as it found it, so every
                    // remaining lap would repeat it exactly: each remaining
                    // child slot is one more zero-width match. Count them
                    // instead of walking them, so a huge `max` can't spin.
                    let lap_was_idle = match &self.divider {
                        None => true,
                        Some(_) => !is_child_slot && child_was_idle,
                    };
                    if lap_was_idle {
                        match_count = self.child_slots();
                        break;
                    }
                    child_was_idle = is_child_slot;
                }
                Some(node) => {
                    child_was_idle = false;
                    self.nodes.push(node);
                    self.cuts_before.push(cut_before);
                    if cursor.at_end() {
                        break;
                    }
                }
            }
        }

        (match_count, kept_cut)
    }

    /// The pattern in `slot` (`Pattern::at_path`): v12's per-slot child
    /// list, child and divider alternating.
    /// How many slots v12 would build (its `children.length`).
    pub(super) fn slot_count(&self) -> usize {
        self.slots
    }

    pub(super) fn slot(&self, slot: usize) -> Option<&Pattern> {
        if slot >= self.slots {
            return None;
        }
        match (&self.divider, slot % 2) {
            (Some(divider), 1) => Some(divider),
            _ => Some(&self.child),
        }
    }

    /// How many of the slots are child slots: with a divider they take the
    /// even indexes, so half the slots rounded up.
    fn child_slots(&self) -> usize {
        match &self.divider {
            None => self.slots,
            Some(_) => self.slots.div_ceil(2),
        }
    }
}
