use std::fmt;

use crate::pattern_error::PatternError;

/// How many composites may be parsing at once when nothing else is said:
/// `Cursor::new` and `Pattern::exec` use it.
///
/// Parsing recurses on the native stack, and through references the depth
/// follows the input (`((((x))))`), so without a bound a deep enough input
/// overflows the stack: a process abort natively, and a wasm module that is
/// broken for every later call. 1024 frames is about 600 KB of stack in a
/// debug build (under 400 KB in release), well inside a 2 MB thread and the
/// 1 MB wasm stack, and still about 340 levels of `expr = "(" expr ")" | x`
/// or 200 of a JSON-style array grammar (serde_json stops at 128).
pub const DEFAULT_MAX_DEPTH: usize = 1024;

/// Limits on one parse. `Limits::default()` is what `exec` uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Limits {
    /// The most composites (sequences, choices, repeats, references, …) that
    /// may be parsing at once. Each one costs native stack, so raise it only
    /// on a thread with the stack to match.
    pub max_depth: usize,
}

impl Default for Limits {
    fn default() -> Limits {
        Limits {
            max_depth: DEFAULT_MAX_DEPTH,
        }
    }
}

/// The input nested deeper than `Limits::max_depth` allows. The parse
/// stopped there: an answer about the input, not a crash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooDeep {
    pub max_depth: usize,
    /// Byte offset where the composite that would have gone too deep began.
    pub pos: usize,
}

impl fmt::Display for TooDeep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the input nests deeper than {} patterns at byte {}; raise `Limits::max_depth` (on a thread with enough stack) to parse it",
            self.max_depth, self.pos
        )
    }
}

impl std::error::Error for TooDeep {}

/// Why a parse stopped before it was done. Once halted, every pattern fails
/// at once and nothing more is recorded, so the parse unwinds without trying
/// alternatives. v12 throws in both cases; this is that throw as data.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Halt {
    /// A reference named nothing (only reachable when `exec`'s up-front
    /// check didn't run or gave up, see `link`).
    Pattern(PatternError),
    TooDeep(TooDeep),
}
