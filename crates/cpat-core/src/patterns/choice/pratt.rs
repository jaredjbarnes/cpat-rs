//! The Pratt mode's parse loop (v12 `Expression._tryToParse`): prefix
//! operators, an operand, postfix operators, then an infix operator and
//! round again, with the precedence tree assembling the result.

use crate::ancestors::At;
use crate::cursor::Cursor;
use crate::name::Name;
use crate::pattern::Pattern;
use crate::pattern_path::step;
use crate::patterns::Sequence;
use crate::tree::NodeId;

use super::already_parsing_here;
use super::precedence::{PrecedenceTree, Table};

/// An operator alternative without its self-references, and the
/// alternative's name, which its nodes carry.
#[derive(Clone, Debug)]
pub(crate) struct Operator {
    // Always a `Pattern::Sequence`, built so by `new`. Held as a `Pattern`
    // so `Pattern::at_path` can hand it out like any other child.
    pattern: Pattern,
    pub(super) name: Name,
    // Its index among the choice's classified children (`Pattern::at_path`);
    // set by `Pratt::new`.
    child: u32,
}

impl Operator {
    pub(super) fn new(sequence: Sequence, name: Name) -> Operator {
        Operator {
            pattern: Pattern::Sequence(sequence),
            name,
            child: 0,
        }
    }

    pub(crate) fn sequence(&self) -> &Sequence {
        match &self.pattern {
            Pattern::Sequence(sequence) => sequence,
            _ => unreachable!("an operator is built from a sequence"),
        }
    }

    fn sequence_mut(&mut self) -> &mut Sequence {
        match &mut self.pattern {
            Pattern::Sequence(sequence) => sequence,
            _ => unreachable!("an operator is built from a sequence"),
        }
    }

    /// An operator is a link in the pattern tree (v12 makes the choice its
    /// parent), so it parses like any composite, in its own frame.
    fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        cursor.set_child(self.child);
        self.pattern.parse(cursor)
    }
}

/// What a choice's alternative became when it was classified (exported as
/// `ChoiceRole`): v12 sorts an `Expression`'s alternatives into
/// `atomPatterns`, `prefixPatterns`, `postfixPatterns` and `infixPatterns`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// An operand: no self-reference at either end, or not a sequence.
    Atom,
    /// `"-" expr`: a sequence whose only self-reference is its last child.
    Prefix,
    /// `expr "!"`: a sequence whose only self-reference is its first child.
    Postfix,
    /// `expr "+" expr`: a sequence that starts and ends with a
    /// self-reference and has something between them.
    Infix,
}

/// One of the choice's classified children (`Pattern::at_path`).
#[derive(Clone, Copy, Debug)]
enum Slot {
    /// The k-th atom.
    Atom(usize),
    Prefix(usize),
    Postfix(usize),
    Infix(usize),
}

#[derive(Clone, Debug)]
pub(crate) struct Pratt {
    /// Indices of the operand alternatives, parsed in place.
    pub(super) atoms: Vec<usize>,
    /// Each atom's index among the classified children.
    atom_children: Vec<u32>,
    pub(super) prefixes: Vec<Operator>,
    pub(super) postfixes: Vec<Operator>,
    pub(super) infixes: Vec<Operator>,
    /// The classified children in declaration order (v12 `children`).
    children: Vec<Slot>,
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
    /// `roles` says what each classified alternative became, in
    /// declaration order; the operators get their child indices from it.
    pub(super) fn new(
        atoms: Vec<usize>,
        mut prefixes: Vec<Operator>,
        mut postfixes: Vec<Operator>,
        mut infixes: Vec<Operator>,
        table: Table,
        roles: &[Role],
    ) -> Pratt {
        let mut atom_children = Vec::with_capacity(atoms.len());
        let mut children = Vec::with_capacity(roles.len());
        let mut counts = [0usize; 4];
        for (index, role) in roles.iter().enumerate() {
            let child = step(index);
            let slot = match role {
                Role::Atom => {
                    atom_children.push(child);
                    Slot::Atom(counts[0])
                }
                Role::Prefix => {
                    prefixes[counts[1]].child = child;
                    Slot::Prefix(counts[1])
                }
                Role::Postfix => {
                    postfixes[counts[2]].child = child;
                    Slot::Postfix(counts[2])
                }
                Role::Infix => {
                    infixes[counts[3]].child = child;
                    Slot::Infix(counts[3])
                }
            };
            counts[*role as usize] += 1;
            children.push(slot);
        }
        Pratt {
            atoms,
            atom_children,
            prefixes,
            postfixes,
            infixes,
            children,
            tree: PrecedenceTree::new(table),
        }
    }

    /// The classified child at `index`: an atom (an alternative) or an
    /// operator's sequence.
    pub(super) fn child<'a>(
        &'a self,
        alternatives: &'a [Pattern],
        index: usize,
    ) -> Option<&'a Pattern> {
        Some(match *self.children.get(index)? {
            Slot::Atom(k) => &alternatives[self.atoms[k]],
            Slot::Prefix(i) => &self.prefixes[i].pattern,
            Slot::Postfix(i) => &self.postfixes[i].pattern,
            Slot::Infix(i) => &self.infixes[i].pattern,
        })
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
            // A literal operator can end inside a grapheme (`++` before a
            // combining mark); the next pattern starts after that
            // grapheme, as everywhere else (v12's `moveTo` then `next`).
            cursor.set_pos(cursor.snap_forward(cursor.node_end(node)));
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
            cursor.set_child(self.atom_children[k]);
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
            // Walked in the operator's own frame, as v12 walks it with the
            // operator as the probes' parent, so a halted probe and the
            // committed error below are recorded under the operator.
            cursor.set_child(operator.child);
            let meta = operator.sequence().meta;
            let sequence = operator.sequence_mut();
            let blocked =
                cursor.framed(meta, |cursor| sequence.child_blocked_at_end_of_text(cursor));
            if cursor.halted() {
                return;
            }
            if let Some(index) = blocked
                && cursor.cut()
            {
                let blocked = operator.sequence().children()[index].meta();
                let end = cursor.pos();
                let below = [operator.child, step(index)];
                cursor.record_error_at(end, end, blocked, At::Below(&below));
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
