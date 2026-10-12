use std::fmt;
use std::rc::Rc;

/// The route from the pattern a parse started at (the exec root) down to one
/// pattern instance: the index of the child taken at each level.
///
/// Patterns keep no parent pointers (DESIGN §2), so a recorded match or error
/// can't point back into the grammar the way v12's `pattern` field does.
/// This path names the same instance instead, and `Pattern::at_path` replays
/// it on the root after the parse. It also stands in for v12's `parent`
/// chain: the instance's ancestors are exactly the prefixes of its path.
///
/// Child indexing per pattern kind is listed on `Pattern::at_path`.
///
/// Cheap to clone (shared): one recorded error is kept as the current error,
/// maybe the furthest one, the error list and the record list at once.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PatternPath(Rc<[u32]>);

impl PatternPath {
    pub fn new(steps: &[u32]) -> PatternPath {
        PatternPath(steps.into())
    }

    /// The path of the exec root itself.
    pub fn root() -> PatternPath {
        PatternPath::new(&[])
    }

    pub fn steps(&self) -> &[u32] {
        &self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for PatternPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "/")?;
        let steps: Vec<String> = self.0.iter().map(u32::to_string).collect();
        write!(f, "{}", steps.join("/"))
    }
}

impl From<&[u32]> for PatternPath {
    fn from(steps: &[u32]) -> PatternPath {
        PatternPath::new(steps)
    }
}

/// A child index as a path step. Every index a parse can reach fits: a
/// bounded repeat's slot, the only index that grows with a bound, advances
/// only while it consumes text, and the text is shorter than `u32::MAX`
/// bytes wherever positions are `u32` (JS). Saturates rather than wraps.
pub(crate) fn step(index: usize) -> u32 {
    u32::try_from(index).unwrap_or(u32::MAX)
}
