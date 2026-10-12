use crate::parse_error::{Match, ParseError};

/// One thing a recording parse saw, in the order it happened (v12
/// `HistoryRecord`, whose `ast`/`error` pair is exactly one of the two).
#[derive(Clone, Debug)]
pub enum HistoryRecord {
    Match(Match),
    Error(ParseError),
}

impl HistoryRecord {
    pub fn pattern_name(&self) -> &'static str {
        match self {
            HistoryRecord::Match(m) => m.pattern.name.as_str(),
            HistoryRecord::Error(e) => e.pattern.name.as_str(),
        }
    }
}

/// What a recording parse keeps (v12 `CursorHistory` while recording):
/// every error, every match, and both in order. Copies, never node
/// references (DESIGN §7c). Exists only on a recording cursor, so an
/// ordinary parse neither allocates nor fills it.
#[derive(Clone, Debug, Default)]
pub(crate) struct History {
    /// v12 stops appending on `stopRecording` but keeps what it has.
    pub(crate) on: bool,
    pub(crate) errors: Vec<ParseError>,
    pub(crate) matches: Vec<Match>,
    pub(crate) records: Vec<HistoryRecord>,
}

impl History {
    pub(crate) fn push_error(&mut self, error: ParseError) {
        self.errors.push(error.clone());
        self.records.push(HistoryRecord::Error(error));
    }

    pub(crate) fn push_match(&mut self, found: Match) {
        self.matches.push(found.clone());
        self.records.push(HistoryRecord::Match(found));
    }

    /// v12 `rootMatch`: the match recorded last. The root, when the parse
    /// matched, since a composite records after its children.
    pub(crate) fn root_match(&self) -> Option<&Match> {
        self.matches.last()
    }
}
