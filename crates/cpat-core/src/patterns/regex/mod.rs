mod matcher;

use crate::cursor::Cursor;
use crate::pattern_error::PatternError;
use crate::pattern_id::Meta;
use crate::regex_translate::ends_with_anchor;
use crate::tree::{NodeId, NodeKind};
use matcher::Matcher;

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
        self.matcher.is_fancy()
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
