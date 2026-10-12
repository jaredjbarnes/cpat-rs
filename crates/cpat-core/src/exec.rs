use crate::cursor::Cursor;
use crate::halt::{Limits, TooDeep};
use crate::history::{History, HistoryRecord};
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
    /// The match that ended furthest, with its path (v12 `leafMatch`).
    pub leaf_match: Option<Match>,
    /// The pattern itself can't run (a reference names nothing). Nothing was
    /// parsed, so the other fields are empty.
    pub pattern_error: Option<PatternError>,
    /// The input nested deeper than `Limits::max_depth`, and the parse
    /// stopped there. No tree; the errors are those recorded up to that point.
    pub too_deep: Option<TooDeep>,
    pub(crate) leaf_matches: Vec<Match>,
    // Only from a recording parse (`Pattern::exec_recording`).
    pub(crate) history: Option<History>,
    pub(crate) text: &'t str,
    /// The limits the parse ran under: the paths in it are as deep as they
    /// allow, and a query about them must accept that depth.
    pub(crate) limits: Limits,
}

impl<'t> ParseResult<'t> {
    /// The text that was parsed.
    pub fn text(&self) -> &'t str {
        self.text
    }

    /// The limits the parse ran under (`Limits::default()` for `exec`).
    pub fn limits(&self) -> Limits {
        self.limits
    }

    /// The pattern can't run: nothing is parsed, only the reason is kept.
    fn refused(text: &'t str, limits: Limits, error: PatternError) -> Self {
        ParseResult {
            tree: None,
            error: None,
            furthest_error: None,
            leaf_match: None,
            pattern_error: Some(error),
            too_deep: None,
            leaf_matches: Vec::new(),
            history: None,
            text,
            limits,
        }
    }

    /// Whether this parse kept a history (v12 `cursor.isRecording`). False
    /// for empty text even when asked: v12 returns before it starts
    /// recording there.
    pub fn is_recording(&self) -> bool {
        self.history.as_ref().is_some_and(|history| history.on)
    }

    /// Every error recorded, in order, each with its path. Empty unless the
    /// parse was recording: an ordinary parse keeps only `error` and
    /// `furthest_error`.
    pub fn errors(&self) -> &[ParseError] {
        self.history.as_ref().map_or(&[], |history| &history.errors)
    }

    /// Every match recorded, in order, each with its path (v12
    /// `allMatchedNodes`/`allMatchedPatterns`). Empty unless recording.
    pub fn matches(&self) -> &[Match] {
        self.history
            .as_ref()
            .map_or(&[], |history| &history.matches)
    }

    /// Errors and matches in the order they were recorded. Empty unless
    /// recording.
    pub fn records(&self) -> &[HistoryRecord] {
        self.history
            .as_ref()
            .map_or(&[], |history| &history.records)
    }

    /// The last match recorded (v12 `rootMatch`). Only while recording.
    pub fn root_match(&self) -> Option<&Match> {
        self.history.as_ref().and_then(History::root_match)
    }

    /// v12 `leafMatches`: the furthest match last, preceded by matches that
    /// ended at the same spot from patterns outside every kept match's
    /// parent chain. Kept in every parse, as in v12.
    pub fn leaf_matches(&self) -> &[Match] {
        &self.leaf_matches
    }
}

/// Whether `exec` keeps a history (v12's `record` argument).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Record {
    No,
    Yes,
}

pub(crate) fn exec<'t>(
    pattern: &mut Pattern,
    text: &'t str,
    limits: Limits,
    record: Record,
) -> ParseResult<'t> {
    let mut cursor = match readied_cursor(pattern, text, limits) {
        Ok(cursor) => cursor,
        Err(error) => return ParseResult::refused(text, limits, error),
    };

    if text.is_empty() {
        return cursor.finish(None);
    }

    if record == Record::Yes {
        cursor.start_recording();
    }

    // v12's `Context.exec` runs its child's exec, so the summary error
    // names the child; see through contexts the same way.
    let (summary, steps) = summary_meta(pattern, Vec::new());

    let root = match pattern.parse(&mut cursor) {
        Some(root) if cursor.node_end(root) == text.len() => Some(root),
        // Matched, but not all of it: the rest of the text is the error.
        Some(root) => {
            cursor.record_summary_error(cursor.node_end(root), text.len(), summary, &steps);
            None
        }
        None => {
            cursor.record_summary_error(0, text.len(), summary, &steps);
            None
        }
    };

    cursor.finish(root)
}

/// v12 AutoComplete's parse: `pattern.parse(cursor)` on a recording
/// cursor, not `exec`. So no summary error is recorded (the cursor's error
/// is whatever the parse left), and the root's node is kept in `tree`
/// whether or not it spans the whole text (v12's `ast`). Empty text is
/// parsed too; AutoComplete answers it before asking.
pub(crate) fn parse_recording<'t>(
    pattern: &mut Pattern,
    text: &'t str,
    limits: Limits,
) -> ParseResult<'t> {
    let mut cursor = match readied_cursor(pattern, text, limits) {
        Ok(cursor) => cursor,
        Err(error) => return ParseResult::refused(text, limits, error),
    };
    cursor.start_recording();
    let root = pattern.parse(&mut cursor);
    cursor.finish(root)
}

/// A cursor over `text` with `pattern`'s references readied, or the reason
/// the pattern can't run. Checked before any shortcut for empty text: a
/// broken pattern is reported whatever the input. (v12 returns before it
/// looks.)
fn readied_cursor<'t>(
    pattern: &mut Pattern,
    text: &'t str,
    limits: Limits,
) -> Result<Cursor<'t>, PatternError> {
    let mut cursor = Cursor::with_limits(text, limits);
    if let Some(scope) = crate::link::prepare(pattern)? {
        cursor.push_scope(ScopeFrame::new(scope, pattern.meta()));
    }
    Ok(cursor)
}

/// The pattern the summary error names, and its path from the root: a
/// context's child is child 0, and a marker is no step at all.
fn summary_meta(pattern: &Pattern, mut steps: Vec<u32>) -> (crate::Meta, Vec<u32>) {
    match pattern {
        Pattern::Context(context) => {
            steps.push(0);
            summary_meta(context.child(), steps)
        }
        // v12's `RightAssociated.exec` is its child's exec.
        Pattern::RightAssociated(marker) => summary_meta(marker.child(), steps),
        _ => (pattern.meta(), steps),
    }
}
