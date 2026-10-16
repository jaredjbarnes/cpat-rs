mod bounded;
mod options;
mod unbounded;

use crate::cursor::Cursor;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
use crate::tree::NodeId;

use bounded::Bounded;
use unbounded::Unbounded;

pub use options::RepeatOptions;

/// `x+`, `x{2,4}`, `(x, ",")+`: matches its child again and again, with an
/// optional divider between the matches.
///
/// The strategy is picked once, when the repeat is built: a repeat with no
/// upper bound loops until the child stops matching; a bounded one walks a
/// fixed number of slots. Parsing never asks which one it is.
#[derive(Clone, Debug)]
pub struct Repeat {
    pub(crate) meta: Meta,
    strategy: Strategy,
    // The bounds as given, before clamping: v12's `Repeat` facade reports
    // these (`min`, `max`) while its implementation parses with the
    // clamped ones (`max` 0 means "exactly min"), so a host showing v12's
    // API needs both.
    requested: (usize, Option<usize>),
}

#[derive(Clone, Debug)]
enum Strategy {
    Unbounded(Unbounded),
    Bounded(Bounded),
}

impl Repeat {
    pub fn new(name: &str, child: Pattern, options: RepeatOptions) -> Repeat {
        // A minimum of 0 is kept: zero occurrences match with no node and
        // no error (v12 raised it to 1 until its `fix(repeat)!` commit).
        let requested = (options.min, options.max);
        let min = options.min;
        let divider = options.divider.map(|d| Box::new(d.adopted()));
        let child = Box::new(child.adopted());

        let strategy = match options.max {
            None => Strategy::Unbounded(Unbounded::new(child, divider, min, options.trim_divider)),
            Some(max) => Strategy::Bounded(Bounded::new(
                child,
                divider,
                min,
                // `max || min` in v12: a zero bound means "exactly min".
                if max == 0 { min } else { max.max(min) },
                options.trim_divider,
            )),
        };

        Repeat {
            meta: Meta::new(name),
            strategy,
            requested,
        }
    }

    /// The fewest matches that count.
    pub fn min(&self) -> usize {
        match &self.strategy {
            Strategy::Unbounded(s) => s.min,
            Strategy::Bounded(s) => s.min,
        }
    }

    /// The most matches, or `None` when unbounded.
    pub fn max(&self) -> Option<usize> {
        match &self.strategy {
            Strategy::Unbounded(_) => None,
            Strategy::Bounded(s) => Some(s.max),
        }
    }

    /// `min` as the options gave it (v12's `Repeat.min`), the same as `min`
    /// now that a minimum of 0 is kept: 0 for the repeat inside a grammar's
    /// `*` or `{,n}`.
    pub fn requested_min(&self) -> usize {
        self.requested.0
    }

    /// `max` as the options gave it, before clamping to `min` (v12's
    /// `Repeat.max`, with `None` for its `Infinity`).
    pub fn requested_max(&self) -> Option<usize> {
        self.requested.1
    }

    pub fn child(&self) -> &Pattern {
        match &self.strategy {
            Strategy::Unbounded(s) => &s.child,
            Strategy::Bounded(s) => &s.child,
        }
    }

    pub fn divider(&self) -> Option<&Pattern> {
        match &self.strategy {
            Strategy::Unbounded(s) => s.divider.as_deref(),
            Strategy::Bounded(s) => s.divider.as_deref(),
        }
    }

    pub fn trim_divider(&self) -> bool {
        match &self.strategy {
            Strategy::Unbounded(s) => s.trim_divider,
            Strategy::Bounded(s) => s.trim_divider,
        }
    }

    /// v12's slot count for a bounded repeat (its `children.length`), `None`
    /// without a bound.
    pub(crate) fn slots(&self) -> Option<usize> {
        match &self.strategy {
            Strategy::Unbounded(_) => None,
            Strategy::Bounded(s) => Some(s.slot_count()),
        }
    }

    /// The child and divider, for walks that may need to change them (the
    /// reference link cache).
    pub(crate) fn parts_mut(&mut self) -> (&mut Pattern, Option<&mut Pattern>) {
        match &mut self.strategy {
            Strategy::Unbounded(s) => (&mut s.child, s.divider.as_deref_mut()),
            Strategy::Bounded(s) => (&mut s.child, s.divider.as_deref_mut()),
        }
    }

    /// The child at `index` (`Pattern::at_path`): `[child, divider]`
    /// without a bound, v12's slots with one.
    pub(crate) fn child_at(&self, index: usize) -> Option<&Pattern> {
        match &self.strategy {
            Strategy::Unbounded(s) => match u32::try_from(index).ok()? {
                unbounded::CHILD => Some(&s.child),
                unbounded::DIVIDER => s.divider.as_deref(),
                _ => None,
            },
            Strategy::Bounded(s) => s.slot(index),
        }
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        match &mut self.strategy {
            Strategy::Unbounded(s) => s.parse(self.meta, cursor),
            Strategy::Bounded(s) => s.parse(self.meta, cursor),
        }
    }
}
