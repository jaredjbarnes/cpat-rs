use crate::cursor::Cursor;
use crate::parse_error::{Match, ParseError};
use crate::pattern::Pattern;
use crate::tree::Tree;

/// The outcome of parsing a whole text.
pub struct ParseResult<'t> {
    /// The tree, when the pattern matched the entire text.
    pub tree: Option<Tree<'t>>,
    /// The error the parse ended on.
    pub error: Option<ParseError>,
    /// The error that got furthest into the text: usually the best diagnosis.
    pub furthest_error: Option<ParseError>,
    pub leaf_match: Option<Match>,
}

pub(crate) fn exec<'t>(pattern: &mut Pattern, text: &'t str) -> ParseResult<'t> {
    let mut cursor = Cursor::new(text);

    if text.is_empty() {
        return cursor.finish(None);
    }

    let root = match pattern.parse(&mut cursor) {
        Some(root) if cursor.node_end(root) == text.len() => Some(root),
        // Matched, but not all of it: the rest of the text is the error.
        Some(root) => {
            cursor.record_summary_error(cursor.node_end(root), text.len(), pattern.meta());
            None
        }
        None => {
            cursor.record_summary_error(0, text.len(), pattern.meta());
            None
        }
    };

    cursor.finish(root)
}
