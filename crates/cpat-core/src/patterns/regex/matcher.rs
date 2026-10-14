use std::rc::Rc;

use crate::pattern_error::PatternError;
use crate::regex_translate::{Dialect, MAX_GROUP_DEPTH, js_to_rust};

/// The engine is chosen once, when the pattern is built: the linear-time
/// `regex` crate unless the translation needs lookaround or backreferences,
/// which only `fancy-regex` supports (`regex_translate::Dialect`).
#[derive(Clone, Debug)]
pub(super) enum Matcher {
    Fast(Rc<regex::Regex>),
    Fancy(Rc<fancy_regex::Regex>),
}

/// How many backtracking steps `fancy-regex` may take on one match before
/// it gives up (and the regex doesn't match). JS has no limit; fancy-regex's
/// default (one million) is about two steps a character for a repeated
/// alternation with a lookahead, so the shell grammar's `dqLiteral` gave up
/// on a 500 KB string that v12 matches (found by the conformance harness,
/// DESIGN §6). A hundred million covers linear patterns over inputs of tens
/// of megabytes, and still ends a catastrophic one, where JS would hang.
const BACKTRACK_LIMIT: usize = 100_000_000;

/// How large one compiled program (and one lazy-DFA cache) may grow, in
/// bytes. Rust writes out every copy of a counted repetition where V8 loops,
/// so a modest JS source is a big Rust program: `.{0,65535}` takes about
/// 90 MB and `\p{L}{1,1000}` about 43 MB, past the crates' 10 MB default,
/// though v12 builds both. 128 MiB covers them with room to spare, and still
/// ends a truly huge program (`a{4294967295}`) as a build error after
/// about 140 MB and a fraction of a second, rather than exhausting memory
/// (DESIGN §6).
const SIZE_LIMIT: usize = 128 << 20;

/// How deeply the `regex` crate lets the translation nest, counting every
/// group, alternation and repetition as a level. Its compiler recurses once
/// a level, so this limit is what keeps a deep source a build error rather
/// than a stack overflow, which would abort the process (or the wasm
/// instance). The crate's default (250) refused 250 plain nested groups,
/// which JS and the translator (`MAX_GROUP_DEPTH`, 256) accept. This admits
/// 256 groups plus the anchoring wrapper and a few levels at the innermost
/// one; the deepest translation it admits compiles in 256 KB of stack in a
/// release build and on a 2 MB test thread in a debug one. A quantified
/// group is three levels (`(?:(…))+`), so 87 of them nest and 88 are refused
/// (DESIGN §6).
const NEST_LIMIT: u32 = MAX_GROUP_DEPTH as u32 + 8;

impl Matcher {
    pub(super) fn compile(name: &str, source: &str) -> Result<Matcher, PatternError> {
        let invalid = |reason: String| PatternError::InvalidRegex {
            name: name.to_string(),
            source: source.to_string(),
            reason,
        };
        let translation = js_to_rust(source).map_err(|error| invalid(error.to_string()))?;
        // Anchored at the start of the slice it runs on. The slice starts at
        // the cursor, so `\b` and lookbehind see the cursor as the start of
        // the input, exactly as the JS library's substring does.
        let anchored = format!("^(?:{})", translation.rust);

        match translation.dialect {
            Dialect::Fast => regex::RegexBuilder::new(&anchored)
                .size_limit(SIZE_LIMIT)
                .dfa_size_limit(SIZE_LIMIT)
                .nest_limit(NEST_LIMIT)
                .build()
                .map(|fast| Matcher::Fast(Rc::new(fast)))
                .map_err(|error| invalid(error.to_string())),
            // fancy-regex parses the source itself, with a fixed depth limit
            // it doesn't let us raise (DESIGN §6); the parts it hands to the
            // `regex` crate get the same size limits as a fast matcher.
            Dialect::Fancy => fancy_regex::RegexBuilder::new(&anchored)
                .backtrack_limit(BACKTRACK_LIMIT)
                .delegate_size_limit(SIZE_LIMIT)
                .delegate_dfa_size_limit(SIZE_LIMIT)
                .build()
                .map(|fancy| Matcher::Fancy(Rc::new(fancy)))
                .map_err(|error| invalid(error.to_string())),
        }
    }

    /// Length in bytes of the match at the start of `haystack`, if any.
    pub(super) fn match_len(&self, haystack: &str) -> Option<usize> {
        match self {
            Matcher::Fast(regex) => regex.find(haystack).map(|m| m.end()),
            // A backtracking-limit error means the engine gave up; that input
            // does not match.
            Matcher::Fancy(regex) => regex.find(haystack).ok().flatten().map(|m| m.end()),
        }
    }

    pub(super) fn is_fancy(&self) -> bool {
        matches!(self, Matcher::Fancy(_))
    }
}
