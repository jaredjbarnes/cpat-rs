use crate::cursor::Cursor;
use crate::halt::{Limits, TooDeep};
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
    /// The input nested deeper than `Limits::max_depth`, and the parse
    /// stopped there. No tree; the errors are those recorded up to that point.
    pub too_deep: Option<TooDeep>,
}

impl ParseResult<'_> {
    /// The pattern can't run: nothing is parsed, only the reason is kept.
    fn refused(error: PatternError) -> Self {
        ParseResult {
            tree: None,
            error: None,
            furthest_error: None,
            leaf_match: None,
            pattern_error: Some(error),
            too_deep: None,
        }
    }
}

pub(crate) fn exec<'t>(pattern: &mut Pattern, text: &'t str, limits: Limits) -> ParseResult<'t> {
    let mut cursor = Cursor::with_limits(text, limits);

    // Checked before the empty-text shortcut: a broken pattern is reported
    // whatever the input. (v12 returns before it looks.)
    match crate::link::prepare(pattern) {
        Ok(Some(scope)) => cursor.push_scope(ScopeFrame::new(scope, pattern.meta())),
        Ok(None) => {}
        Err(error) => return ParseResult::refused(error),
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
        // v12's `RightAssociated.exec` is its child's exec.
        Pattern::RightAssociated(marker) => summary_meta(marker.child()),
        _ => pattern.meta(),
    }
}
