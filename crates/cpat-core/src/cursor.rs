use std::rc::Rc;

use crate::ancestors::{Ancestors, Frame};
use crate::graphemes::Boundaries;
use crate::name::Name;
use crate::parse_error::{Match, ParseError};
use crate::pattern::Pattern;
use crate::pattern_error::PatternError;
use crate::pattern_id::{Meta, PatternId};
use crate::scope::{self, ScopeFrame};
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
    leaf_match: Option<Match>,
    choice_scopes: Vec<bool>,
    ancestors: Ancestors,
    scopes: Vec<ScopeFrame>,
    pattern_error: Option<PatternError>,
    tree: TreeBuilder,
}

impl<'t> Cursor<'t> {
    pub fn new(text: &'t str) -> Cursor<'t> {
        Cursor {
            text,
            pos: 0,
            stopped_at: None,
            boundaries: Boundaries::new(text),
            error: None,
            furthest_error: None,
            leaf_match: None,
            choice_scopes: Vec::new(),
            ancestors: Ancestors::default(),
            scopes: Vec::new(),
            pattern_error: None,
            tree: TreeBuilder::default(),
        }
    }

    /// An empty-text cursor for asking "would this match nothing?" (what a
    /// lookahead sees at the very end) without touching this parse. It keeps
    /// the scopes, so references inside still resolve, and the ancestors, so
    /// recursion guards see the same chain v12's parent pointers would.
    pub(crate) fn probe(&self) -> Cursor<'static> {
        let mut probe = Cursor::new("");
        probe.ancestors = self.ancestors.clone();
        probe.scopes = self.scopes.clone();
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
        self.error
    }

    pub fn furthest_error(&self) -> Option<ParseError> {
        self.furthest_error
    }

    pub fn leaf_match(&self) -> Option<Match> {
        self.leaf_match
    }

    pub fn record_error(&mut self, start: usize, last: usize, pattern: Meta) {
        let error = ParseError {
            start,
            last,
            pattern,
        };
        self.error = Some(error);

        if self
            .furthest_error
            .is_none_or(|furthest| last > furthest.last)
        {
            self.furthest_error = Some(error);
        }
    }

    /// Records an error that summarizes the whole parse ("this didn't match
    /// all of the text") rather than a pattern's own failure. Its `last` is
    /// the text length, which would beat every real failure, so it only
    /// becomes the furthest error when no pattern recorded one.
    pub(crate) fn record_summary_error(&mut self, start: usize, last: usize, pattern: Meta) {
        let furthest = self.furthest_error;
        self.record_error(start, last, pattern);
        if furthest.is_some() {
            self.furthest_error = furthest;
        }
    }

    pub fn resolve_error(&mut self) {
        self.error = None;
    }

    pub fn record_match(&mut self, pattern: Meta, start: usize, end: usize) {
        if self.leaf_match.is_none_or(|leaf| end > leaf.end) {
            self.leaf_match = Some(Match {
                pattern,
                start,
                end,
            });
        }
    }

    /// A grammar mistake met while parsing (an unknown reference name, when
    /// `parse` is driven directly). `exec` checks for these before it parses.
    pub fn pattern_error(&self) -> Option<&PatternError> {
        self.pattern_error.as_ref()
    }

    pub(crate) fn record_pattern_error(&mut self, error: PatternError) {
        self.pattern_error.get_or_insert(error);
    }

    // ─── Ancestors (DESIGN §2) ──────────────────────────────────────────

    /// The composites parsing right now, outermost first.
    pub fn ancestors(&self) -> &[Frame] {
        self.ancestors.frames()
    }

    /// Runs `f` inside an ancestor frame for the composite `id`, starting
    /// here. Always popped: parsing fails only by returning.
    #[inline]
    pub(crate) fn framed<R>(&mut self, id: PatternId, f: impl FnOnce(&mut Self) -> R) -> R {
        self.ancestors.push(id, self.pos);
        let result = f(self);
        self.ancestors.pop();
        result
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

    /// The master `name` resolves to here, innermost scope first.
    pub(crate) fn lookup(&self, name: Name) -> Option<Rc<Pattern>> {
        scope::lookup(&self.scopes, name).map(|found| found.master)
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

    pub(crate) fn finish(self, root: Option<NodeId>) -> crate::ParseResult<'t> {
        crate::ParseResult {
            tree: root.map(|root| self.tree.finish(self.text, root)),
            error: self.error,
            furthest_error: self.furthest_error,
            leaf_match: self.leaf_match,
            pattern_error: self.pattern_error,
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
