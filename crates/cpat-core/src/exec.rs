use crate::cursor::Cursor;
use crate::parse_error::{Match, ParseError};
use crate::pattern::Pattern;
use crate::pattern_error::PatternError;
use crate::scope::ScopeFrame;
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
    /// The pattern itself can't run (a reference names nothing). Nothing was
    /// parsed, so the other fields are empty.
    pub pattern_error: Option<PatternError>,
}

pub(crate) fn exec<'t>(pattern: &mut Pattern, text: &'t str) -> ParseResult<'t> {
    let mut cursor = Cursor::new(text);

    // Checked before the empty-text shortcut: a broken pattern is reported
    // whatever the input. (v12 returns before it looks.)
    match crate::link::prepare(pattern) {
        Ok(Some(scope)) => cursor.push_scope(ScopeFrame::new(scope, pattern.meta())),
        Ok(None) => {}
        Err(error) => {
            cursor.record_pattern_error(error);
            return cursor.finish(None);
        }
    }

    if text.is_empty() {
        return cursor.finish(None);
    }

    // v12's `Context.exec` runs its child's exec, so the summary error
    // names the child; see through contexts the same way.
    let summary = summary_meta(pattern);

    let root = match pattern.parse(&mut cursor) {
        Some(root) if cursor.node_end(root) == text.len() => Some(root),
        // Matched, but not all of it: the rest of the text is the error.
        Some(root) => {
            cursor.record_summary_error(cursor.node_end(root), text.len(), summary);
            None
        }
        None => {
            cursor.record_summary_error(0, text.len(), summary);
            None
        }
    };

    cursor.finish(root)
}

fn summary_meta(pattern: &Pattern) -> crate::Meta {
    match pattern {
        Pattern::Context(context) => summary_meta(context.child()),
        _ => pattern.meta(),
    }
}
