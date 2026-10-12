//! What a pattern could offer (v12 `_getCompositeSuggestionsForPattern`):
//! its leaf patterns' tokens, custom tokens by name, and, for a greedy
//! name, each of those joined with whatever could follow it.

use std::cell::Cell;
use std::collections::HashMap;

use crate::introspect::{Handle, Introspection, leaf_tokens};

use super::options::Dedupe;
use super::suggestion::{Composite, SegmentPattern};

/// How many greedy joins may nest. v12 recurses without limit, so a greedy
/// pattern whose continuation leads back to it (a greedy child of a
/// repeat, followed by itself; `list = item sep list?` with `item` and
/// `sep` greedy, through a reference) recurses until the JS engine throws.
/// Here the innermost join offers nothing, so the whole chain does, as a
/// greedy pattern with nothing after it does in v12. Each level joins one
/// more token; a real suggestion joins a handful.
const MAX_GREEDY_DEPTH: usize = 32;

/// How many places the greedy joins of one composite suggestion may
/// expand. Joins multiply: a cycle that branches makes twice the places
/// per level, so the depth limit alone could still run for hours (2^32
/// places). Counted, never timed, so the answer is the same on every
/// machine. Past it a place offers nothing. Only places inside a join
/// count: without one, a pattern expands to its leaves once each, which is
/// linear in the grammar (a choice of 50 000 keywords is not cut short).
/// v12's tests join at most a few dozen places.
const BUDGET: usize = 10_000;

pub(super) struct Composer<'a, 'r> {
    pub(super) intro: &'a Introspection<'r>,
    pub(super) greedy: &'a [String],
    pub(super) custom_tokens: &'a HashMap<String, Vec<String>>,
    pub(super) dedupe: Dedupe,
    spent: Cell<usize>,
}

impl<'a, 'r> Composer<'a, 'r> {
    pub(super) fn new(
        intro: &'a Introspection<'r>,
        greedy: &'a [String],
        custom_tokens: &'a HashMap<String, Vec<String>>,
        dedupe: Dedupe,
    ) -> Composer<'a, 'r> {
        Composer {
            intro,
            greedy,
            custom_tokens,
            dedupe,
            spent: Cell::new(0),
        }
    }

    /// The composite suggestions for the pattern at `handle`. Each call
    /// gets the whole budget.
    pub(super) fn for_pattern(&self, handle: &Handle<'r>) -> Vec<Composite> {
        self.spent.set(0);
        self.compose(handle, 0)
    }

    /// `joins` greedy joins enclose this call.
    fn compose(&self, handle: &Handle<'r>, joins: usize) -> Vec<Composite> {
        if joins > 0 && !self.spend() {
            return Vec::new();
        }
        let custom = self.custom(handle);
        let own = match leaf_tokens(handle.pattern) {
            // A leaf's patterns are itself: it offers its custom tokens,
            // then its own. (v12 asks `getPatterns` and compares ids; for a
            // leaf that is always so, and asking by path would walk the
            // path from the root once per leaf.)
            Some(tokens) => custom
                .into_iter()
                .chain(tokens)
                .map(|token| Composite::single(token, segment_pattern(handle)))
                .collect(),
            None => self.composite_own(handle, custom, joins),
        };

        if !self.greedy.iter().any(|name| name == handle.name()) {
            return self.dedupe.apply(own);
        }
        self.join_with_next(handle, own, joins)
    }

    /// A pattern that isn't a leaf offers its custom tokens, then its leaf
    /// patterns'. (v12 treats a pattern whose patterns are just itself as a
    /// leaf; only a leaf's are.)
    fn composite_own(
        &self,
        handle: &Handle<'r>,
        custom: Vec<String>,
        joins: usize,
    ) -> Vec<Composite> {
        let mut own: Vec<Composite> = custom
            .into_iter()
            .map(|token| Composite::single(token, segment_pattern(handle)))
            .collect();
        for leaf in self.intro.patterns(&handle.path) {
            own.extend(self.compose(&leaf, joins));
        }
        own
    }

    /// v12's greedy join: every suggestion followed by every suggestion of
    /// every pattern that may come next. Nothing may come next → nothing,
    /// as in v12. Not deduped, as in v12.
    fn join_with_next(
        &self,
        handle: &Handle<'r>,
        own: Vec<Composite>,
        joins: usize,
    ) -> Vec<Composite> {
        if joins >= MAX_GREEDY_DEPTH {
            return Vec::new();
        }
        let mut following = Vec::new();
        for next in self.intro.next_patterns(&handle.path) {
            following.extend(self.compose(&next, joins + 1));
        }

        let mut joined = Vec::with_capacity(own.len() * following.len());
        for first in &own {
            for then in &following {
                let mut sequence = first.sequence.clone();
                sequence.extend(then.sequence.iter().cloned());
                joined.push(Composite {
                    text: format!("{}{}", first.text, then.text),
                    sequence,
                });
            }
        }
        joined
    }

    fn custom(&self, handle: &Handle<'r>) -> Vec<String> {
        self.custom_tokens
            .get(handle.name())
            .cloned()
            .unwrap_or_default()
    }

    fn spend(&self) -> bool {
        let spent = self.spent.get();
        self.spent.set(spent + 1);
        spent < BUDGET
    }
}

pub(super) fn segment_pattern(handle: &Handle<'_>) -> SegmentPattern {
    SegmentPattern {
        path: handle.path.clone(),
        name: handle.name(),
        id: handle.id(),
    }
}
