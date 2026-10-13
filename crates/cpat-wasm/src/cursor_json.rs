//! What v12's cursor tells about a parse (`error`, `furthestError`,
//! `leafMatch`, `errors`, `records`, `rootMatch`), as one JSON text in
//! UTF-16 offsets. `js/cursor.mjs` reads it once, when a cursor property is
//! first asked for, so a parse whose cursor nobody reads pays only for the
//! text.

use cpat_core::{HistoryRecord, Match, ParseError, ParseResult, PatternPath};
use serde_json::{Value, json};

use crate::utf16::Utf16Offsets;

/// `{ error, furthestError, leafMatches, errors, records, rootMatch,
/// isRecording }`. An error is `{ start, last, name, id, path }` (`last`
/// inclusive, as v12's `lastIndex`; `path` is `null` outside a recording,
/// DESIGN §15); a match is `{ start, end, name, id, path }`. Paths lead
/// from the root that was parsed.
pub(crate) fn cursor_json(result: &ParseResult<'_>, offsets: &Utf16Offsets) -> String {
    let error = |error: &ParseError| error_json(error, offsets);
    let found = |found: &Match| match_json(found, offsets);
    let records: Vec<Value> = result
        .records()
        .iter()
        .map(|record| match record {
            HistoryRecord::Match(m) => json!({ "match": found(m) }),
            HistoryRecord::Error(e) => json!({ "error": error(e) }),
        })
        .collect();

    json!({
        "error": result.error.as_ref().map(error),
        "furthestError": result.furthest_error.as_ref().map(error),
        "leafMatches": result.leaf_matches().iter().map(found).collect::<Vec<_>>(),
        "errors": result.errors().iter().map(error).collect::<Vec<_>>(),
        "records": records,
        "rootMatch": result.root_match().map(found),
        "isRecording": result.is_recording(),
    })
    .to_string()
}

fn error_json(error: &ParseError, offsets: &Utf16Offsets) -> Value {
    json!({
        "start": offsets.of(error.start),
        "last": offsets.of(error.last),
        "name": error.pattern.name.as_str(),
        "id": error.pattern.id.to_string(),
        "path": error.path.as_ref().map(PatternPath::steps),
    })
}

fn match_json(found: &Match, offsets: &Utf16Offsets) -> Value {
    json!({
        "start": offsets.of(found.start),
        "end": offsets.of(found.end),
        "name": found.pattern.name.as_str(),
        "id": found.pattern.id.to_string(),
        "path": found.path.steps(),
    })
}
