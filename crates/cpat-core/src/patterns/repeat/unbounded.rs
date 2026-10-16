use crate::ancestors::At;
use crate::cursor::Cursor;
use crate::leaves::Parent;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
use crate::tree::{NodeId, NodeKind};

// The child's and the divider's indices (`Pattern::at_path`), as v12's
// `InfiniteRepeat` lists them.
pub(super) const CHILD: u32 = 0;
pub(super) const DIVIDER: u32 = 1;

/// A repeat with no upper bound: child, divider, child, ... until something
/// stops matching. Mirrors v12's `InfiniteRepeat`.
#[derive(Clone, Debug)]
pub(super) struct Unbounded {
    pub(super) child: Box<Pattern>,
    pub(super) divider: Option<Box<Pattern>>,
    pub(super) min: usize,
    pub(super) trim_divider: bool,
    // Scratch space for one parse, reused across parses. Safe because a
    // pattern instance is never re-entered: recursion reaches a fresh clone.
    nodes: Vec<NodeId>,
    // Parallel to `nodes`: whether each node came from the divider, so a
    // trailing divider can be recognised and trimmed.
    from_divider: Vec<bool>,
    // Cut bookkeeping (only kept work commits): `cuts_before[i]` is the cut
    // flag just before `nodes[i]` was parsed, so trimming that node can undo
    // its cut; `kept_cut` is the flag as of the last step that succeeded.
    cuts_before: Vec<bool>,
    kept_cut: bool,
}

impl Unbounded {
    pub(super) fn new(
        child: Box<Pattern>,
        divider: Option<Box<Pattern>>,
        min: usize,
        trim_divider: bool,
    ) -> Unbounded {
        Unbounded {
            child,
            divider,
            min,
            trim_divider,
            nodes: Vec::new(),
            from_divider: Vec::new(),
            cuts_before: Vec::new(),
            kept_cut: false,
        }
    }

    pub(super) fn parse(&mut self, meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();
        self.nodes.clear();
        self.from_divider.clear();
        self.cuts_before.clear();
        self.kept_cut = cursor.cut();

        // On failure the cut flag is left as the failed work set it, as in
        // v12: the failure propagates, so the enclosing choice decides.
        // With a minimum of 0 nothing failed: zero occurrences match, with
        // no node, and the dropped work's error and cut go with it.
        if !self.parse_items(meta, cursor, first) {
            if self.min == 0 {
                cursor.resolve_error();
                cursor.set_cut(self.kept_cut);
                cursor.set_pos(first);
            }
            return None;
        }

        cursor.resolve_error();
        let node = self.finish(meta, cursor, first);

        // A failed trailing iteration or divider (and a trimmed divider) is
        // dropped from the result, and so is any cut it fired.
        cursor.set_cut(self.kept_cut);
        node
    }

    /// Runs the loop, then checks the minimum. `true` when what was matched
    /// is kept; `false` with an error is a failure, and `false` without one
    /// is v12's "nothing to keep" outcome.
    fn parse_items(&mut self, meta: Meta, cursor: &mut Cursor<'_>, first: usize) -> bool {
        let passed = self.run_loop(cursor, first);

        if self.meets_min() || !passed {
            return passed;
        }

        // Matched something, but not enough. This is the only failure that
        // is the repeat's own; every other one belongs to the child.
        cursor.record_error_at(first, cursor.last_index(), meta, At::Frame);
        cursor.set_pos(first);
        false
    }

    /// Alternates child and divider until one of them stops it. `true` when
    /// the loop ended in a way that keeps the nodes gathered so far.
    fn run_loop(&mut self, cursor: &mut Cursor<'_>, first: usize) -> bool {
        loop {
            let running = cursor.pos();
            let cut_before = cursor.cut();
            cursor.set_child(CHILD);
            let node = self.child.parse(cursor);

            if cursor.has_error() {
                if !self.nodes.is_empty() {
                    return true;
                }

                // Nothing kept: the failure is the child's, reported from
                // where the repeat started to where this attempt began.
                cursor.set_pos(running);
                cursor.record_error_at(first, running, self.child.meta(), At::Below(&[CHILD]));
                return false;
            }

            self.kept_cut = cursor.cut();

            match node {
                Some(node) => {
                    self.keep(node, false, cut_before);
                    if cursor.at_end() {
                        return true;
                    }
                }
                None => {
                    // The cursor stays where the child left it, as in v12
                    // (only the bounded strategy rewinds a slot). A child can
                    // succeed without a node and still have moved: an inner
                    // unbounded repeat over an `Optional` does, and the
                    // divider is then tried from there.
                    //
                    // A zero-width child with no divider would match the
                    // same nothing forever: stop. v12 reports this as "no
                    // result, no error", whatever was kept before.
                    if self.divider.is_none() {
                        return false;
                    }
                }
            }

            let Some(divider) = self.divider.as_mut() else {
                continue;
            };

            let divider_start = cursor.pos();
            let cut_before_divider = cursor.cut();
            cursor.set_child(DIVIDER);
            let divider_node = divider.parse(cursor);

            if cursor.has_error() {
                return true;
            }

            self.kept_cut = cursor.cut();

            match divider_node {
                Some(divider_node) => {
                    self.keep(divider_node, true, cut_before_divider);
                    if cursor.at_end() {
                        return true;
                    }
                }
                None => {
                    cursor.set_pos(divider_start);
                    // Neither the child nor the divider consumed anything:
                    // another lap would do the same.
                    if node.is_none() {
                        return true;
                    }
                }
            }
        }
    }

    fn keep(&mut self, node: NodeId, from_divider: bool, cut_before: bool) {
        self.nodes.push(node);
        self.from_divider.push(from_divider);
        self.cuts_before.push(cut_before);
    }

    /// With a divider, `nodes` interleaves matches and dividers, so the
    /// matches are half of them, rounded up (v12's `ceil(nodes / 2)`).
    fn meets_min(&self) -> bool {
        let matches = match self.divider {
            Some(_) => self.nodes.len().div_ceil(2),
            None => self.nodes.len(),
        };
        matches >= self.min
    }

    fn finish(&mut self, meta: Meta, cursor: &mut Cursor<'_>, first: usize) -> Option<NodeId> {
        if self.trim_divider && self.from_divider.last() == Some(&true) {
            self.nodes.pop();
            self.from_divider.pop();
            if let Some(cut) = self.cuts_before.pop() {
                self.kept_cut = cut;
            }
        }

        let Some(&last) = self.nodes.last() else {
            cursor.set_pos(first);
            return None;
        };

        // The node spans from where the repeat started, as in v12.
        let end = cursor.node_end(last);
        let node = cursor.branch(NodeKind::InfiniteRepeat, meta.name, first, end, &self.nodes);
        cursor.record_match_at(meta, first, end, At::Frame, Parent::Facade);
        cursor.set_pos(cursor.snap_forward(end));
        Some(node)
    }
}
