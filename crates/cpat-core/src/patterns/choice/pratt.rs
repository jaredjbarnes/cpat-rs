//! The Pratt mode's parse loop (v12 `Expression._tryToParse`): prefix
//! operators, an operand, postfix operators, then an infix operator and
//! round again, with the precedence tree assembling the result.

use crate::cursor::Cursor;
use crate::name::Name;
use crate::pattern::Pattern;
use crate::patterns::Sequence;
use crate::tree::NodeId;

use super::already_parsing_here;
use super::precedence::{PrecedenceTree, Table};

/// An operator alternative without its self-references, and the
/// alternative's name, which its nodes carry.
#[derive(Clone, Debug)]
pub(crate) struct Operator {
    pub(crate) sequence: Sequence,
    pub(super) name: Name,
}

impl Operator {
    /// What `Pattern::parse` does for a sequence. An operator is a link in
    /// the pattern tree (v12 makes the choice its parent), so it gets a
    /// frame like any composite.
    fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        if cursor.halted() {
            return None;
        }
        let sequence = &mut self.sequence;
        cursor.framed(sequence.meta, |cursor| sequence.parse(cursor))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct Pratt {
    /// Indices of the operand alternatives, parsed in place.
    pub(super) atoms: Vec<usize>,
    pub(super) prefixes: Vec<Operator>,
    pub(super) postfixes: Vec<Operator>,
    pub(super) infixes: Vec<Operator>,
    tree: PrecedenceTree,
}

/// Where one run of the loop stands.
#[derive(Default)]
struct Progress {
    /// Nothing more can be added: the text ended, a phase found nothing,
    /// or a commitment failed.
    stop: bool,
    /// A cut fired inside an attempt that failed: the choice fails
    /// committed.
    cut_fired: bool,
}

impl Pratt {
    pub(super) fn new(
        atoms: Vec<usize>,
        prefixes: Vec<Operator>,
        postfixes: Vec<Operator>,
        infixes: Vec<Operator>,
        table: Table,
    ) -> Pratt {
        Pratt {
            atoms,
            prefixes,
            postfixes,
            infixes,
            tree: PrecedenceTree::new(table),
        }
    }

    /// Runs inside the choice's cut scope. Returns the assembled tree (the
    /// cursor just past it), or whether the failure is committed.
    pub(super) fn run(
        &mut self,
        alternatives: &mut [Pattern],
        cursor: &mut Cursor<'_>,
    ) -> (Option<NodeId>, bool) {
        let mut progress = Progress::default();

        loop {
            cursor.resolve_error();

            self.match_prefixes(cursor, &mut progress);
            if progress.stop {
                break;
            }
            self.match_atom(alternatives, cursor, &mut progress);
            if progress.stop {
                break;
            }
            self.match_postfixes(cursor, &mut progress);
            if progress.stop {
                break;
            }
            if !self.tree.has_atom() {
                break;
            }
            self.match_infix(cursor, &mut progress);
            if progress.stop {
                break;
            }
        }

        if progress.cut_fired {
            // A committed alternative failed. Whatever was assembled is
            // discarded: reading the committed region as something shorter
            // is exactly the rewind the cut forbids.
            self.tree.reset();
            return (None, true);
        }

        let node = self.tree.commit(cursor);
        if let Some(node) = node {
            cursor.set_pos(cursor.node_end(node));
        }
        (node, false)
    }

    fn match_prefixes(&mut self, cursor: &mut Cursor<'_>, progress: &mut Progress) {
        let mut on = cursor.pos();
        let mut i = 0;

        while i < self.prefixes.len() {
            let operator = &mut self.prefixes[i];
            match operator.parse(cursor) {
                Some(node) => {
                    // A success discharges any commitment it made; left set,
                    // it would turn a later failed probe into a committed
                    // failure.
                    cursor.clear_cut();
                    self.tree
                        .add_prefix(operator.name, cursor.node_children(node));
                    if cursor.at_end() {
                        // Known limitation (v12's): the prefix keeps the
                        // last token even though no operand can follow.
                        progress.stop = true;
                        return;
                    }
                    // Prefixes stack (`- - x`): start over from the first.
                    on = cursor.pos();
                    i = 0;
                    continue;
                }
                None => {
                    cursor.set_pos(on);
                    if check_cut(cursor, progress) {
                        return;
                    }
                    cursor.resolve_error();
                }
            }
            i += 1;
        }
    }

    fn match_atom(
        &mut self,
        alternatives: &mut [Pattern],
        cursor: &mut Cursor<'_>,
        progress: &mut Progress,
    ) {
        let on = cursor.pos();

        for k in 0..self.atoms.len() {
            cursor.set_pos(on);
            let atom = &mut alternatives[self.atoms[k]];

            if already_parsing_here(cursor, atom, on) {
                continue;
            }

            match atom.parse(cursor) {
                Some(node) => {
                    cursor.clear_cut();
                    self.tree.add_atom(node);
                    if cursor.at_end() {
                        progress.stop = true;
                        self.try_operators_at_end(cursor, progress);
                    }
                    return;
                }
                None => {
                    if check_cut(cursor, progress) {
                        return;
                    }
                    cursor.resolve_error();
                    cursor.set_pos(on);
                }
            }
        }
    }

    fn match_postfixes(&mut self, cursor: &mut Cursor<'_>, progress: &mut Progress) {
        let mut on = cursor.pos();
        let mut i = 0;

        while i < self.postfixes.len() {
            let operator = &mut self.postfixes[i];
            match operator.parse(cursor) {
                Some(node) => {
                    cursor.clear_cut();
                    self.tree
                        .add_postfix(operator.name, cursor.node_children(node));
                    if cursor.at_end() {
                        progress.stop = true;
                        self.try_operators_at_end(cursor, progress);
                        return;
                    }
                    // Postfixes stack (`a!!`): start over from the first.
                    on = cursor.pos();
                    i = 0;
                    continue;
                }
                None => {
                    cursor.set_pos(on);
                    if check_cut(cursor, progress) {
                        return;
                    }
                    cursor.resolve_error();
                }
            }
            i += 1;
        }
    }

    fn match_infix(&mut self, cursor: &mut Cursor<'_>, progress: &mut Progress) {
        let on = cursor.pos();

        for operator in &mut self.infixes {
            cursor.set_pos(on);
            match operator.parse(cursor) {
                Some(node) => {
                    cursor.clear_cut();
                    self.tree
                        .add_binary(operator.name, cursor.node_children(node));
                    // A trailing operator with no right operand is undone
                    // at commit.
                    progress.stop = cursor.at_end();
                    return;
                }
                None => {
                    cursor.set_pos(on);
                    if check_cut(cursor, progress) {
                        return;
                    }
                    cursor.resolve_error();
                }
            }
        }

        // No infix operator continues the expression.
        progress.stop = true;
    }

    /// An operand just ended the text, so the loop stops without trying
    /// the postfix and infix operators. Mid-text, trying them fires a cut
    /// that leads an operator (`e ~/ "+" e`); with no text left that cut is
    /// reached all the same, so it fires here too, and the committed
    /// operator records what it still needed, at the end of the text.
    /// Nothing is recorded unless a cut fires (v12
    /// `_tryOperatorsAtEndOfText`).
    fn try_operators_at_end(&mut self, cursor: &mut Cursor<'_>, progress: &mut Progress) {
        for operator in self.postfixes.iter_mut().chain(self.infixes.iter_mut()) {
            let blocked = operator
                .sequence
                .child_blocked_at_end_of_text(cursor)
                .map(Pattern::meta);
            if cursor.halted() {
                return;
            }
            if let Some(blocked) = blocked
                && cursor.cut()
            {
                let end = cursor.pos();
                cursor.record_error(end, end, blocked);
                check_cut(cursor, progress);
                return;
            }
        }
    }
}

/// A cut fired inside the attempt that just failed: stop trying, and fail
/// the whole choice committed.
fn check_cut(cursor: &Cursor<'_>, progress: &mut Progress) -> bool {
    let committed = cursor.cut();
    if committed {
        progress.cut_fired = true;
        progress.stop = true;
    }
    committed
}
