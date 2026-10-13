use std::rc::Rc;

use crate::cursor::Cursor;
use crate::pattern_error::PatternError;
use crate::pattern_id::Meta;
use crate::regex_translate::{Dialect, ends_with_anchor, js_to_rust};
use crate::tree::{NodeId, NodeKind};

/// Matches a regular expression at the cursor.
///
/// The source is JavaScript regex syntax, as written in `.cpat` files; it is
/// translated to Rust syntax with the same meaning when the pattern is built.
#[derive(Clone, Debug)]
pub struct Regex {
    pub(crate) meta: Meta,
    source: Box<str>,
    matcher: Matcher,
    // What intellisense suggests for it (`Tokenizable`).
    pub(crate) tokens: Vec<String>,
}

/// The engine is chosen once, when the pattern is built: the linear-time
/// `regex` crate unless the translation needs lookaround or backreferences,
/// which only `fancy-regex` supports (`regex_translate::Dialect`).
#[derive(Clone, Debug)]
enum Matcher {
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

impl Matcher {
    fn compile(name: &str, source: &str) -> Result<Matcher, PatternError> {
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
            Dialect::Fast => regex::Regex::new(&anchored)
                .map(|fast| Matcher::Fast(Rc::new(fast)))
                .map_err(|error| invalid(error.to_string())),
            Dialect::Fancy => fancy_regex::RegexBuilder::new(&anchored)
                .backtrack_limit(BACKTRACK_LIMIT)
                .build()
                .map(|fancy| Matcher::Fancy(Rc::new(fancy)))
                .map_err(|error| invalid(error.to_string())),
        }
    }

    /// Length in bytes of the match at the start of `haystack`, if any.
    fn match_len(&self, haystack: &str) -> Option<usize> {
        match self {
            Matcher::Fast(regex) => regex.find(haystack).map(|m| m.end()),
            // A backtracking-limit error means the engine gave up; that input
            // does not match.
            Matcher::Fancy(regex) => regex.find(haystack).ok().flatten().map(|m| m.end()),
        }
    }
}

impl Regex {
    pub fn new(name: &str, source: &str) -> Result<Regex, PatternError> {
        if source.is_empty() {
            return Err(PatternError::EmptyRegex {
                name: name.to_string(),
            });
        }

        if source.starts_with('^') || ends_with_anchor(source) {
            return Err(PatternError::AnchoredRegex {
                name: name.to_string(),
                source: source.to_string(),
            });
        }

        Ok(Regex {
            meta: Meta::new(name),
            source: source.into(),
            matcher: Matcher::compile(name, source)?,
            tokens: Vec::new(),
        })
    }

    /// The source as written, before translation.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Whether this regex needed the backtracking engine.
    pub fn is_fancy(&self) -> bool {
        matches!(self.matcher, Matcher::Fancy(_))
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();

        match self.matcher.match_len(cursor.rest()) {
            // An empty match is not a match: it would let a repeat spin forever.
            Some(len) if len > 0 => {
                // The match may end inside a grapheme; the node takes the whole
                // grapheme, and its value is a slice of the text it covers, so
                // value and range always agree.
                let end = cursor.snap_forward(first + len);
                cursor.resolve_error();
                let node = cursor.leaf(NodeKind::Regex, self.meta.name, first, end);
                cursor.record_match(self.meta, first, end);
                cursor.set_pos(end);
                Some(node)
            }
            _ => {
                cursor.record_error(first, first, self.meta);
                None
            }
        }
    }
}
