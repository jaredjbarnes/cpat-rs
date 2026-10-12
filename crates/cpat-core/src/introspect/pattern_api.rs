//! Introspection on a pattern as a root: the questions v12 asks of a
//! pattern with no parent, and the parent-dependent ones by path.

use crate::pattern::Pattern;
use crate::pattern_path::PatternPath;

use super::{Handle, Introspection};

impl Pattern {
    /// Introspection of this pattern's tree, with this pattern as the root
    /// every path starts from. Build one to ask several questions.
    pub fn introspect(&self) -> Introspection<'_> {
        Introspection::new(self)
    }

    /// What can start this pattern (v12 `getTokens`).
    pub fn tokens(&self) -> Vec<String> {
        self.introspect().tokens(&PatternPath::root())
    }

    /// The leaf patterns that can start this pattern (v12 `getPatterns`).
    pub fn patterns(&self) -> Vec<Handle<'_>> {
        self.introspect().patterns(&PatternPath::root())
    }

    /// The tokens that may follow this pattern's child `child` (v12
    /// `getTokensAfter`), with this pattern as the root: it has no parent,
    /// so nothing follows it.
    pub fn tokens_after(&self, child: usize) -> Vec<String> {
        self.introspect().tokens_after(&PatternPath::root(), child)
    }

    /// The patterns that may follow this pattern's child `child` (v12
    /// `getPatternsAfter`), with this pattern as the root.
    pub fn patterns_after(&self, child: usize) -> Vec<Handle<'_>> {
        self.introspect()
            .patterns_after(&PatternPath::root(), child)
    }

    /// The tokens that may follow the instance at `path` (v12
    /// `getNextTokens` on it): v12 asks its parent, which is the path's
    /// prefix here.
    pub fn next_tokens(&self, path: &PatternPath) -> Vec<String> {
        self.introspect().next_tokens(path)
    }

    /// The patterns that may follow the instance at `path` (v12
    /// `getNextPatterns` on it).
    pub fn next_patterns(&self, path: &PatternPath) -> Vec<Handle<'_>> {
        self.introspect().next_patterns(path)
    }

    /// The first pattern in this one's tree that `predicate` accepts, with
    /// its path (v12 `find`): depth first, children before their parent,
    /// never into a reference. Literals, regexes, take-untils and
    /// references find nothing, not even themselves, as in v12.
    pub fn find(&self, predicate: impl Fn(&Pattern) -> bool) -> Option<Handle<'_>> {
        Introspection::new(self).find(predicate)
    }
}
