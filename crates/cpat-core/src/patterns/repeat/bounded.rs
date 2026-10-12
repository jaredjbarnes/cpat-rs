use crate::cursor::Cursor;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
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
        let slots = match (&divider, trim_divider) {
            (None, _) => max,
            (Some(_), false) => 2 * max,
            (Some(_), true) => 2 * max - 1,
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
            // Empty when nothing matched at all. v12 throws a TypeError
            // there; with nothing to trim, this falls through to the
            // minimum check like the untrimmed repeat does.
            if let Some(node) = self.nodes.pop() {
                kept_cut = self.cuts_before.pop().unwrap_or(kept_cut);
                cursor.set_pos(cursor.node_start(node));
            }
        }

        if match_count < self.min {
            let last = cursor.last_index();
            cursor.set_pos(first);
            cursor.record_error(first, last, meta);
            return None;
        }

        // From here on the repeat succeeds: a failed trailing slot (and a
        // trimmed divider) is dropped from the result, and so is any cut it
        // fired.
        cursor.set_cut(kept_cut);

        let (Some(&first_node), Some(&last_node)) = (self.nodes.first(), self.nodes.last()) else {
            // Only zero-width matches. v12 returns "no node, no error" here,
            // and throws a TypeError when a later slot failed; both become a
            // zero-width success, the failed slot dropped like any other.
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

        for slot in 0..self.slots {
            let is_child_slot = self.divider.is_none() || slot % 2 == 0;
            let pattern = match (&mut self.divider, is_child_slot) {
                (Some(divider), false) => divider,
                _ => &mut self.child,
            };

            let running = cursor.pos();
            let cut_before = cursor.cut();
            let node = pattern.parse(cursor);

            if cursor.has_error() {
                break;
            }

            kept_cut = cursor.cut();
            if is_child_slot {
                match_count += 1;
            }

            match node {
                None => cursor.set_pos(running),
                Some(node) => {
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
}
