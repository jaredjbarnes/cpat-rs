use crate::ancestors::{Ancestors, At, Frame};
use crate::graphemes::Boundaries;
use crate::halt::{Halt, Limits, TooDeep};
use crate::history::{History, HistoryRecord};
use crate::leaves::{Leaves, Parent};
use crate::name::Name;
use crate::parse_error::{Match, ParseError};
use crate::pattern_error::PatternError;
use crate::pattern_id::{Meta, PatternId};
use crate::pattern_path::PatternPath;
use crate::scope::{self, Found, ScopeFrame};
use crate::tree::{NodeId, NodeKind, Tree, TreeBuilder};

/// The state of one parse: position, errors, cut scopes, the ancestor and
/// scope stacks, and the node arena.
///
/// Positions are byte offsets on grapheme boundaries, half-open: after a
/// pattern matches, the cursor sits just past the match, where the next
/// pattern starts. End of text is `pos() == len()`.
pub struct Cursor<'t> {
    text: &'t str,
    pos: usize,
    // Where clarity-pattern-parser's cursor would sit after the latest
    // failure, when that isn't `pos`. See `last_index`.
    stopped_at: Option<usize>,
    boundaries: Boundaries,
    error: Option<ParseError>,
    furthest_error: Option<ParseError>,
    leaves: Leaves,
    // Only on a recording cursor (`start_recording`), so the branch on it
    // is decided once per parse and an ordinary parse allocates nothing
    // for history (DESIGN §7c).
    history: Option<Box<History>>,
    choice_scopes: Vec<bool>,
    ancestors: Ancestors,
    scopes: Vec<ScopeFrame>,
    // Lookups see `scopes[scope_floor..]`: a target parsing at its home
    // (`within_scopes`) must not see the frames of the place it was
    // reached from.
    scope_floor: usize,
    max_depth: usize,
    halt: Option<Halt>,
    tree: TreeBuilder,
}

impl<'t> Cursor<'t> {
    pub fn new(text: &'t str) -> Cursor<'t> {
        Cursor::with_limits(text, Limits::default())
    }

    pub fn with_limits(text: &'t str, limits: Limits) -> Cursor<'t> {
        Cursor {
            text,
            pos: 0,
            stopped_at: None,
            boundaries: Boundaries::new(text),
            error: None,
            furthest_error: None,
            leaves: Leaves::default(),
            history: None,
            choice_scopes: Vec::new(),
            ancestors: Ancestors::default(),
            scopes: Vec::new(),
            scope_floor: 0,
            max_depth: limits.max_depth,
            halt: None,
            tree: TreeBuilder::default(),
        }
    }

    /// An empty-text cursor for asking "would this match nothing?" (what a
    /// lookahead sees at the very end) without touching this parse. It keeps
    /// the scopes, so references inside still resolve, and the ancestors, so
    /// recursion guards see the same chain v12's parent pointers would.
    pub(crate) fn probe(&self) -> Cursor<'static> {
        let mut probe = Cursor::with_limits(
            "",
            Limits {
                max_depth: self.max_depth,
            },
        );
        probe.ancestors = self.ancestors.clone();
        probe.scopes = self.scopes.clone();
        probe.scope_floor = self.scope_floor;
        probe
    }

    // ─── Position ───────────────────────────────────────────────────────

    pub fn text(&self) -> &'t str {
        self.text
    }

    pub fn len(&self) -> usize {
        self.text.len()
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    pub fn pos(&self) -> usize {
        self.pos
    }

    pub fn at_end(&self) -> bool {
        self.pos >= self.text.len()
    }

    /// The position as clarity-pattern-parser's inclusive cursor would report
    /// it. Errors recorded "where the cursor stopped" (a repeat's own `min`
    /// failure) use this, so their `last` matches v12.
    ///
    /// Usually that is `pos`, except at the end of the text, where that cursor
    /// can't go past the last grapheme and sits on its start instead. And
    /// right after some failures v12's cursor is not back where the failed
    /// pattern began: a `Literal` stays on the character that didn't match,
    /// and a `Sequence` that ran out of text stays at the end. Here a failed
    /// pattern always leaves `pos` where it began (nothing parses from those
    /// spots), and notes v12's spot with `stopped_at`; the next move forgets it.
    pub(crate) fn last_index(&self) -> usize {
        if let Some(stopped_at) = self.stopped_at {
            return stopped_at;
        }
        if self.at_end() && !self.text.is_empty() {
            self.boundaries.snap_back(self.text.len() - 1)
        } else {
            self.pos
        }
    }

    /// Notes where v12's cursor would have stopped after the failure just
    /// recorded, for `last_index`. `pos` doesn't move.
    pub(crate) fn stopped_at(&mut self, index: usize) {
        self.stopped_at = Some(index);
    }

    /// The unparsed rest of the text. Free: it is a slice.
    pub fn rest(&self) -> &'t str {
        &self.text[self.pos..]
    }

    /// Moves to `pos`, which callers take from node ends or earlier positions,
    /// so it is already on a boundary.
    pub fn set_pos(&mut self, pos: usize) {
        debug_assert!(
            self.boundaries.is_boundary(pos),
            "{pos} is not a grapheme boundary"
        );
        self.pos = pos.min(self.text.len());
        self.stopped_at = None;
    }

    /// The first grapheme boundary at or after `i`.
    pub fn snap_forward(&self, i: usize) -> usize {
        self.boundaries.snap_forward(i.min(self.text.len()))
    }

    /// The last grapheme boundary at or before `i`.
    pub fn snap_back(&self, i: usize) -> usize {
        self.boundaries.snap_back(i.min(self.text.len()))
    }

    // ─── Errors and matches ─────────────────────────────────────────────

    pub fn has_error(&self) -> bool {
        self.error.is_some()
    }

    pub fn error(&self) -> Option<ParseError> {
        self.error.clone()
    }

    pub fn furthest_error(&self) -> Option<ParseError> {
        self.furthest_error.clone()
    }

    /// The match that ended furthest (v12 `leafMatch`), with its path.
    pub fn leaf_match(&self) -> Option<Match> {
        self.leaves.furthest()
    }

    /// v12 `leafMatches`: the furthest match last, preceded by matches that
    /// ended at the same spot and that no kept match's parent chain
    /// already covers (see `Leaves`).
    pub fn leaf_matches(&self) -> Vec<Match> {
        self.leaves.to_matches()
    }

    /// Records a failure of `pattern`, a pattern with no frame of its own:
    /// a leaf, as a `Custom` pattern is.
    pub fn record_error(&mut self, start: usize, last: usize, pattern: Meta) {
        self.record_error_at(start, last, pattern, At::Child);
    }

    /// Records a failure of the pattern `at` names (it decides the path a
    /// recording parse keeps).
    pub(crate) fn record_error_at(&mut self, start: usize, last: usize, pattern: Meta, at: At<'_>) {
        // A halted parse keeps the error it halted on (see `halt`).
        if self.halt.is_some() {
            return;
        }
        let path = self.recorded_path(at);
        let error = ParseError {
            start,
            last,
            pattern,
            path,
        };

        if self
            .furthest_error
            .as_ref()
            .is_none_or(|furthest| last > furthest.last)
        {
            self.furthest_error = Some(error.clone());
        }
        if let Some(history) = self.recording_history() {
            history.push_error(error.clone());
        }
        self.error = Some(error);
    }

    /// Records an error that summarizes the whole parse ("this didn't match
    /// all of the text") rather than a pattern's own failure. Its `last` is
    /// the text length, which would beat every real failure, so it only
    /// becomes the furthest error when no pattern recorded one. `steps`
    /// lead from the root to the pattern it names.
    pub(crate) fn record_summary_error(
        &mut self,
        start: usize,
        last: usize,
        pattern: Meta,
        steps: &[u32],
    ) {
        let furthest = self.furthest_error.take();
        self.record_error_at(start, last, pattern, At::Below(steps));
        if furthest.is_some() {
            self.furthest_error = furthest;
        }
    }

    /// Clears the current error: the failure was expected (an optional
    /// child, a lookahead, an alternative that lost). Not after a halt:
    /// the error stays, so every parent fails in turn.
    pub fn resolve_error(&mut self) {
        if self.halt.is_none() {
            self.error = None;
        }
    }

    /// Records a match of `pattern`, a pattern with no frame of its own: a
    /// leaf, as a `Custom` pattern is.
    pub fn record_match(&mut self, pattern: Meta, start: usize, end: usize) {
        self.record_match_at(pattern, start, end, At::Child, Parent::Above);
    }

    /// Records a match of the pattern `at` names, whose v12 parent is
    /// `parent` (only the leaf matches' tie rule reads it).
    pub(crate) fn record_match_at(
        &mut self,
        pattern: Meta,
        start: usize,
        end: usize,
        at: At<'_>,
        parent: Parent,
    ) {
        self.leaves
            .record(pattern, start, end, &self.ancestors, at, parent);
        if self.is_recording() {
            let path = self.path_of(at);
            let found = Match {
                pattern,
                start,
                end,
                path,
            };
            if let Some(history) = self.recording_history() {
                history.push_match(found);
            }
        }
    }

    // ─── Recording (v12 `CursorHistory`) ────────────────────────────────

    /// From now on, every error and match is kept, in order, with the path
    /// of the pattern instance that produced it. `exec_recording` calls
    /// this before parsing.
    pub fn start_recording(&mut self) {
        self.history.get_or_insert_with(Default::default).on = true;
    }

    /// Stops adding to the history; what it holds stays readable.
    pub fn stop_recording(&mut self) {
        if let Some(history) = &mut self.history {
            history.on = false;
        }
    }

    pub fn is_recording(&self) -> bool {
        self.history.as_ref().is_some_and(|history| history.on)
    }

    /// Every error recorded while recording, in order (v12 `errors`).
    pub fn errors(&self) -> &[ParseError] {
        self.history.as_ref().map_or(&[], |history| &history.errors)
    }

    /// Every match recorded while recording, in order (v12
    /// `allMatchedNodes` and `allMatchedPatterns` in one list).
    pub fn matches(&self) -> &[Match] {
        self.history
            .as_ref()
            .map_or(&[], |history| &history.matches)
    }

    /// Errors and matches together, in the order they were recorded.
    pub fn records(&self) -> &[HistoryRecord] {
        self.history
            .as_ref()
            .map_or(&[], |history| &history.records)
    }

    /// The last match recorded while recording (v12 `rootMatch`).
    pub fn root_match(&self) -> Option<&Match> {
        self.history
            .as_ref()
            .and_then(|history| history.root_match())
    }

    fn recording_history(&mut self) -> Option<&mut History> {
        self.history.as_deref_mut().filter(|history| history.on)
    }

    /// The path a new error keeps: only a recording parse builds one.
    fn recorded_path(&self, at: At<'_>) -> Option<PatternPath> {
        self.is_recording().then(|| self.path_of(at))
    }

    fn path_of(&self, at: At<'_>) -> PatternPath {
        let mut steps = Vec::new();
        self.ancestors.path_into(at, &mut steps);
        PatternPath::new(&steps)
    }

    // ─── Halting ────────────────────────────────────────────────────────

    /// The parse can't go on at all (see `Halt`). Every pattern checks this
    /// on entry and fails, and errors stop changing, so the parse unwinds
    /// straight out instead of trying alternatives.
    pub fn halted(&self) -> bool {
        self.halt.is_some()
    }

    /// A grammar mistake met while parsing: an unknown reference name, when
    /// `parse` is driven directly. `exec` checks for these before it parses.
    pub fn pattern_error(&self) -> Option<&PatternError> {
        match &self.halt {
            Some(Halt::Pattern(error)) => Some(error),
            _ => None,
        }
    }

    /// The input nested deeper than the cursor's `max_depth`.
    pub fn too_deep(&self) -> Option<TooDeep> {
        match self.halt {
            Some(Halt::TooDeep(too_deep)) => Some(too_deep),
            _ => None,
        }
    }

    /// Records the failure of `pattern` (named by `at`) at `pos` as the
    /// last error, then halts.
    pub(crate) fn halt(&mut self, pos: usize, pattern: Meta, at: At<'_>, halt: Halt) {
        self.record_error_at(pos, pos, pattern, at);
        self.halt.get_or_insert(halt);
    }

    /// Carries a probe's halt over: a lookahead's child that halted halts
    /// the real parse too, at the lookahead's position. The lookahead is
    /// the top frame's current child.
    pub(crate) fn halt_from(&mut self, probe: Cursor<'_>, pattern: Meta) {
        let halt = match probe.halt {
            Some(Halt::TooDeep(too_deep)) => Halt::TooDeep(TooDeep {
                pos: self.pos,
                ..too_deep
            }),
            Some(halt) => halt,
            None => return,
        };
        self.halt(self.pos, pattern, At::Child, halt);
    }

    // ─── Ancestors (DESIGN §2) ──────────────────────────────────────────

    /// The composites parsing right now, outermost first.
    pub fn ancestors(&self) -> &[Frame] {
        self.ancestors.frames()
    }

    /// Runs `f` inside an ancestor frame for the composite `meta`, starting
    /// here. Always popped: parsing fails only by returning. A frame past
    /// `max_depth` halts the parse instead (see `Limits`): the stack is the
    /// one resource a parse can't get back once it overflows.
    #[inline]
    pub(crate) fn framed<R>(
        &mut self,
        meta: Meta,
        f: impl FnOnce(&mut Self) -> Option<R>,
    ) -> Option<R> {
        if self.ancestors.len() >= self.max_depth {
            let too_deep = TooDeep {
                max_depth: self.max_depth,
                pos: self.pos,
            };
            // Refused before its frame exists: it is the top frame's child.
            self.halt(self.pos, meta, At::Child, Halt::TooDeep(too_deep));
            return None;
        }
        self.ancestors.push(meta.id, self.pos);
        let result = f(self);
        self.ancestors.pop();
        result
    }

    /// The top composite moves on to its child `index` (in
    /// `Pattern::at_path`'s numbering), so whatever records next is recorded
    /// under the right path.
    #[inline]
    pub(crate) fn set_child(&mut self, index: u32) {
        self.ancestors.set_child(index);
    }

    /// How many active composites with `id` began at `pos`, up to `limit`.
    pub(crate) fn ancestors_at(&self, id: PatternId, pos: usize, limit: usize) -> usize {
        self.ancestors.count_at(id, pos, limit)
    }

    // ─── Scopes (DESIGN §3) ─────────────────────────────────────────────

    pub(crate) fn push_scope(&mut self, frame: ScopeFrame) {
        self.scopes.push(frame);
    }

    pub(crate) fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// What `name` resolves to here, innermost scope first.
    pub(crate) fn lookup(&self, name: Name) -> Option<Found> {
        scope::lookup(&self.scopes[self.scope_floor..], name)
    }

    /// Whether a target's `home` still applies here (`scope::home_applies`).
    pub(crate) fn home_applies(&self, home: &[ScopeFrame]) -> bool {
        scope::home_applies(home, &self.scopes[self.scope_floor..])
    }

    /// Runs `f` with `frames` as the whole scope stack, then restores the
    /// stack. Pushes onto the same stack under a raised floor rather than
    /// swapping in a new one, so it doesn't allocate once warm.
    pub(crate) fn within_scopes<R>(
        &mut self,
        frames: &[ScopeFrame],
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let floor = std::mem::replace(&mut self.scope_floor, self.scopes.len());
        self.scopes.extend(frames.iter().cloned());
        let result = f(self);
        self.scopes.truncate(self.scope_floor);
        self.scope_floor = floor;
        result
    }

    // ─── Nodes ──────────────────────────────────────────────────────────

    pub fn leaf(&mut self, kind: NodeKind, name: Name, start: usize, end: usize) -> NodeId {
        self.tree.leaf(kind, name, start, end)
    }

    pub fn branch(
        &mut self,
        kind: NodeKind,
        name: Name,
        start: usize,
        end: usize,
        children: &[NodeId],
    ) -> NodeId {
        self.tree.branch(kind, name, start, end, children)
    }

    pub fn node_start(&self, id: NodeId) -> usize {
        self.tree.start(id)
    }

    pub fn node_end(&self, id: NodeId) -> usize {
        self.tree.end(id)
    }

    pub fn node_children(&self, id: NodeId) -> &[NodeId] {
        self.tree.children(id)
    }

    // ─── Committed choice (cut, `~/`) ───────────────────────────────────
    // Every choice runs its alternatives inside a scope. A `Cut` marks the
    // innermost scope; the choice checks the mark after a failed alternative
    // and, when set, fails outright instead of trying siblings.

    /// Runs `f` inside a fresh choice scope. The scope is always popped:
    /// parsing cannot fail except by returning `None`, so there is no early
    /// exit to guard against beyond `f` returning.
    pub fn in_choice_scope<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.choice_scopes.push(false);
        let result = f(self);
        self.choice_scopes.pop();
        result
    }

    /// Commits the innermost choice. With no enclosing choice it does nothing:
    /// a cut with nothing to commit is harmless.
    pub fn mark_cut(&mut self) {
        self.set_cut(true);
    }

    /// Whether the innermost choice is committed.
    pub fn cut(&self) -> bool {
        self.choice_scopes.last().copied().unwrap_or(false)
    }

    /// Discharges the innermost choice's commitment: an attempt that
    /// SUCCEEDED keeps nothing to answer for. A choice in Pratt mode calls
    /// this after every operand or operator it keeps, so a cut fired inside
    /// a success can't turn a later failed probe (no further operator) into
    /// a committed failure.
    pub fn clear_cut(&mut self) {
        self.set_cut(false);
    }

    /// Restores a saved commitment. Patterns that throw away a child's work
    /// (Optional, Not) put back the flag they saw before the child ran, so a
    /// cut inside discarded work never commits anything: only kept work
    /// commits.
    pub fn set_cut(&mut self, cut: bool) {
        if let Some(top) = self.choice_scopes.last_mut() {
            *top = cut;
        }
    }

    // ─── Finishing ──────────────────────────────────────────────────────

    /// Ends the parse and keeps the tree rooted at `root`. For callers that
    /// drive `parse` directly instead of through `exec`.
    pub fn into_tree(self, root: NodeId) -> Tree<'t> {
        self.tree.finish(self.text, root)
    }

    /// Ends the parse. A halted parse has no tree, whatever `root` says.
    pub(crate) fn finish(self, root: Option<NodeId>) -> crate::ParseResult<'t> {
        let (pattern_error, too_deep, root) = match self.halt {
            None => (None, None, root),
            Some(Halt::Pattern(error)) => (Some(error), None, None),
            Some(Halt::TooDeep(too_deep)) => (None, Some(too_deep), None),
        };
        crate::ParseResult {
            tree: root.map(|root| self.tree.finish(self.text, root)),
            error: self.error,
            furthest_error: self.furthest_error,
            leaf_match: self.leaves.furthest(),
            pattern_error,
            too_deep,
            leaf_matches: self.leaves.to_matches(),
            history: self.history.map(|history| *history),
            text: self.text,
        }
    }
}

impl<'t> crate::ParseResult<'t> {
    pub fn is_match(&self) -> bool {
        self.tree.is_some()
    }

    pub fn tree(&self) -> Option<&Tree<'t>> {
        self.tree.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::{Context, Literal};

    fn defines_u() -> ScopeFrame {
        let u = Literal::new("u", "u").unwrap().into();
        let child = Literal::new("c", "c").unwrap().into();
        Context::new("k", child, vec![u]).frame()
    }

    #[test]
    fn a_target_at_home_sees_only_its_home_frames() {
        // The frames of the place a target was reached from must not leak
        // into its home: lookups stop at the floor, and so do the probes
        // a lookahead makes there.
        let u = Name::new("u");
        let mut cursor = Cursor::new("x");
        cursor.push_scope(defines_u());

        let seen = cursor.within_scopes(&[], |cursor| {
            (
                cursor.lookup(u).is_some(),
                cursor.probe().lookup(u).is_some(),
            )
        });
        let home = cursor.within_scopes(&[defines_u()], |cursor| cursor.lookup(u).is_some());

        assert_eq!(seen, (false, false));
        assert!(home);
        assert!(cursor.lookup(u).is_some(), "the stack is restored");
    }
}
