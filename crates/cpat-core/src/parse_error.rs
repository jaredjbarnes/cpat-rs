use crate::pattern_id::Meta;
use crate::pattern_path::PatternPath;

/// Where a pattern failed.
///
/// `start` is where the pattern began; `last` is the furthest byte offset it
/// reached before failing, inclusive. `last` can equal the text length when
/// the input ended while the pattern still needed more, which is how an
/// "ended too early" failure outranks one at the last character.
///
/// This is deliberately not a half-open range like node positions: it records
/// how far the attempt got, and the furthest-error rule compares exactly that.
#[derive(Clone, Debug)]
pub struct ParseError {
    pub start: usize,
    pub last: usize,
    pub pattern: Meta,
    /// Which instance failed, from the exec root (see `PatternPath`). Only a
    /// recording parse captures it: an ordinary parse records an error at
    /// almost every failed attempt, and a path per error would cost each one
    /// an allocation that only the recording's readers want.
    pub path: Option<PatternPath>,
}

/// A match recorded by the cursor: a copy, not a node reference, because the
/// node may belong to an alternative that later failed (DESIGN §7c).
#[derive(Clone, Debug)]
pub struct Match {
    pub pattern: Meta,
    pub start: usize,
    pub end: usize,
    /// Which instance matched, from the exec root (see `PatternPath`).
    pub path: PatternPath,
}
