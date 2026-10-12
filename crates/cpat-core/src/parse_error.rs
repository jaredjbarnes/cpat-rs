use crate::pattern_id::Meta;

/// Where a pattern failed.
///
/// `start` is where the pattern began; `last` is the furthest byte offset it
/// reached before failing, inclusive. `last` can equal the text length when
/// the input ended while the pattern still needed more, which is how an
/// "ended too early" failure outranks one at the last character.
///
/// This is deliberately not a half-open range like node positions: it records
/// how far the attempt got, and the furthest-error rule compares exactly that.
#[derive(Clone, Copy, Debug)]
pub struct ParseError {
    pub start: usize,
    pub last: usize,
    pub pattern: Meta,
}

/// The furthest-reaching match seen so far: a copy, not a node reference,
/// because the node may belong to an alternative that later failed.
#[derive(Clone, Copy, Debug)]
pub struct Match {
    pub pattern: Meta,
    pub start: usize,
    pub end: usize,
}
