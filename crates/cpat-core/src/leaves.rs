use crate::ancestors::{Ancestors, At};
use crate::parse_error::Match;
use crate::pattern_id::Meta;
use crate::pattern_path::PatternPath;

/// The furthest matches (v12 `CursorHistory.leafMatches`): the match that
/// ended furthest, preceded by other matches that ended at the same spot
/// whose parent is no ancestor of a match already kept.
///
/// v12 keeps this in every parse, recording or not, and so does this: the
/// last entry is `leaf_match`, which error messages and autocomplete start
/// from, so it carries its path even in an ordinary parse.
///
/// **Cost.** A path is copied only when a match ties or beats the furthest
/// end; matches that end earlier (most of them) cost one comparison. Paths
/// live in buffers that are recycled when a further match clears the list,
/// so once warm an ordinary parse copies paths without allocating.
#[derive(Clone, Debug, Default)]
pub(crate) struct Leaves {
    /// Furthest last; ties are put in front (v12 `unshift`).
    entries: Vec<Leaf>,
    spare: Vec<Vec<u32>>,
    scratch: Vec<u32>,
}

#[derive(Clone, Debug)]
struct Leaf {
    pattern: Meta,
    start: usize,
    end: usize,
    path: Vec<u32>,
    parent: Parent,
}

/// Where the recording pattern's v12 `parent` is, for the tie rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Parent {
    /// The pattern above it in the path.
    Above,
    /// Its own position: v12 wraps every repeat in a `Repeat` facade whose
    /// only child is the repeat that parses and records, so that repeat's
    /// parent is a pattern at the same path. The facade is an ancestor of
    /// everything inside the repeat, and nothing else.
    Facade,
}

impl Leaves {
    #[inline]
    pub(crate) fn record(
        &mut self,
        pattern: Meta,
        start: usize,
        end: usize,
        ancestors: &Ancestors,
        at: At<'_>,
        parent: Parent,
    ) {
        let furthest = self.entries.last().map(|leaf| leaf.end);
        match furthest {
            Some(furthest) if end < furthest => {}
            Some(furthest) if end == furthest => {
                ancestors.path_into(at, &mut self.scratch);
                if !self.parent_encloses_a_leaf(parent) {
                    let mut path = self.spare.pop().unwrap_or_default();
                    path.clone_from(&self.scratch);
                    let leaf = Leaf {
                        pattern,
                        start,
                        end,
                        path,
                        parent,
                    };
                    self.entries.insert(0, leaf);
                }
            }
            _ => {
                // A new furthest match replaces the list. Usually that is
                // one entry, overwritten in place; the rest give their
                // buffers back for later ties.
                if self.entries.len() > 1 {
                    let keep = self.entries.pop();
                    self.spare
                        .extend(self.entries.drain(..).map(|leaf| leaf.path));
                    self.entries.extend(keep);
                }
                match self.entries.last_mut() {
                    Some(leaf) => {
                        leaf.pattern = pattern;
                        leaf.start = start;
                        leaf.end = end;
                        leaf.parent = parent;
                        ancestors.path_into(at, &mut leaf.path);
                    }
                    None => {
                        let mut path = Vec::new();
                        ancestors.path_into(at, &mut path);
                        self.entries.push(Leaf {
                            pattern,
                            start,
                            end,
                            path,
                            parent,
                        });
                    }
                }
            }
        }
    }

    /// v12's tie rule: the new match is dropped when its parent is an
    /// ancestor of a match already kept (so a pattern closing over a kept
    /// leaf, at the same end, adds nothing). With paths, the ancestors of a
    /// kept match are the patterns at the proper prefixes of its path, plus
    /// its own facade when it is a repeat. A pattern with no parent (the
    /// exec root, unless it is a repeat) is never dropped.
    fn parent_encloses_a_leaf(&self, parent: Parent) -> bool {
        let path = &self.scratch[..];
        match parent {
            // The facade at `path`: above everything inside the repeat, and
            // the parent of the repeat itself.
            Parent::Facade => self.entries.iter().any(|leaf| {
                leaf.path.starts_with(path)
                    && (leaf.path.len() > path.len() || leaf.parent == Parent::Facade)
            }),
            Parent::Above => match path.split_last() {
                Some((_, above)) => self
                    .entries
                    .iter()
                    .any(|leaf| leaf.path.len() > above.len() && leaf.path.starts_with(above)),
                None => false,
            },
        }
    }

    /// v12 `leafMatch`: the furthest.
    pub(crate) fn furthest(&self) -> Option<Match> {
        self.entries.last().map(Leaf::to_match)
    }

    /// v12 `leafMatches`, furthest last.
    pub(crate) fn to_matches(&self) -> Vec<Match> {
        self.entries.iter().map(Leaf::to_match).collect()
    }
}

impl Leaf {
    fn to_match(&self) -> Match {
        Match {
            pattern: self.pattern,
            start: self.start,
            end: self.end,
            path: PatternPath::new(&self.path),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(leaves: &Leaves) -> Vec<&'static str> {
        leaves
            .to_matches()
            .iter()
            .map(|m| m.pattern.name.as_str())
            .collect()
    }

    /// Frames whose children spell `path`, under a root frame.
    fn at(path: &[u32]) -> Ancestors {
        let mut ancestors = Ancestors::default();
        for &step in path {
            ancestors.push(Meta::new("frame").id, 0);
            ancestors.set_child(step);
        }
        ancestors
    }

    #[test]
    fn a_kept_match_is_not_its_own_ancestor() {
        // v12 walks a kept match's ancestors from its parent up, so a
        // child of a kept pattern, ending where it did, is kept too.
        let mut leaves = Leaves::default();
        leaves.record(Meta::new("p"), 0, 5, &at(&[1]), At::Child, Parent::Above);
        leaves.record(Meta::new("c"), 2, 5, &at(&[1, 0]), At::Child, Parent::Above);

        assert_eq!(names(&leaves), ["c", "p"]);
    }

    #[test]
    fn a_repeat_tying_with_itself_is_dropped_by_its_facade() {
        // The facade is the repeat's parent and so its own ancestor.
        let mut leaves = Leaves::default();
        leaves.record(Meta::new("r"), 0, 5, &at(&[1]), At::Child, Parent::Facade);
        leaves.record(Meta::new("r"), 1, 5, &at(&[1]), At::Child, Parent::Facade);
        // A sibling repeat's facade is no ancestor of it.
        leaves.record(Meta::new("q"), 2, 5, &at(&[2]), At::Child, Parent::Facade);

        assert_eq!(names(&leaves), ["q", "r"]);
    }
}
