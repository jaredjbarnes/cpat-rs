use crate::graphemes::Boundaries;
use crate::name::Name;
use crate::parse_error::{Match, ParseError};
use crate::pattern_id::Meta;
use crate::tree::{NodeId, NodeKind, Tree, TreeBuilder};

/// The state of one parse: position, errors, cut scopes and the node arena.
///
/// Positions are byte offsets on grapheme boundaries, half-open: after a
/// pattern matches, the cursor sits just past the match, where the next
/// pattern starts. End of text is `pos() == len()`.
pub struct Cursor<'t> {
    text: &'t str,
    pos: usize,
    boundaries: Boundaries,
    error: Option<ParseError>,
    furthest_error: Option<ParseError>,
    leaf_match: Option<Match>,
    choice_scopes: Vec<bool>,
    tree: TreeBuilder,
}

impl<'t> Cursor<'t> {
    pub fn new(text: &'t str) -> Cursor<'t> {
        Cursor {
            text,
            pos: 0,
            boundaries: Boundaries::new(text),
            error: None,
            furthest_error: None,
            leaf_match: None,
            choice_scopes: Vec::new(),
            tree: TreeBuilder::default(),
        }
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
    /// it: `pos`, except at the end of the text, where that cursor can't go
    /// past the last grapheme and sits on its start instead. Errors recorded
    /// "where the cursor stopped" use this, so their `last` matches v12.
    pub(crate) fn last_index(&self) -> usize {
        if self.at_end() && !self.text.is_empty() {
            self.boundaries.snap_back(self.text.len() - 1)
        } else {
            self.pos
        }
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
