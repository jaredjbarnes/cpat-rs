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
}

#[derive(Clone, Debug)]
enum Strategy {
    Unbounded(Unbounded),
    Bounded(Bounded),
}

impl Repeat {
    pub fn new(name: &str, child: Pattern, options: RepeatOptions) -> Repeat {
        // v12 clamps in both implementations: zero-or-more is an optional
        // around one-or-more, never a repeat with min 0.
        let min = options.min.max(1);
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
        }
    }

    /// The fewest matches that count, after clamping to at least 1.
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

    /// The child and divider, for walks that may need to change them (the
    /// reference link cache).
    pub(crate) fn parts_mut(&mut self) -> (&mut Pattern, Option<&mut Pattern>) {
        match &mut self.strategy {
            Strategy::Unbounded(s) => (&mut s.child, s.divider.as_deref_mut()),
            Strategy::Bounded(s) => (&mut s.child, s.divider.as_deref_mut()),
        }
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        match &mut self.strategy {
            Strategy::Unbounded(s) => s.parse(self.meta, cursor),
            Strategy::Bounded(s) => s.parse(self.meta, cursor),
        }
    }
}
