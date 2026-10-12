//! Fitting suggestions to text the user already typed (v12
//! `_trimSuggestionsByExistingText`, `_filterCompletedSubSegments`).

use super::suggestion::{Composite, SuggestionSegment};

/// Keeps the suggestions that start with `current` and cuts `current` off
/// their text, dropping any left with nothing to add. Their segments lose
/// the ones `current` already completed.
///
/// `current = "abc"`, sequence `[ab][cd][ef]` → text `"def"`, segments
/// `[cd][ef]`: `[cd]` was only started, so it keeps its full token.
pub(super) fn trim_by_existing_text(current: &str, composites: Vec<Composite>) -> Vec<Composite> {
    composites
        .into_iter()
        .filter_map(|composite| {
            let rest = composite.text.strip_prefix(current)?;
            if rest.is_empty() {
                return None;
            }
            Some(Composite {
                text: rest.to_string(),
                sequence: without_completed(current, composite.sequence),
            })
        })
        .collect()
}

/// v12 walks the segments once, consuming `current` with each segment it
/// starts with. A segment it doesn't start with stays and consumes nothing,
/// so a later one can still match (v12's quirk, kept: the same text gives
/// the same segments in both).
fn without_completed(current: &str, segments: Vec<SuggestionSegment>) -> Vec<SuggestionSegment> {
    let mut working = current;
    segments
        .into_iter()
        .filter(
            |segment| match working.strip_prefix(segment.text.as_str()) {
                Some(rest) => {
                    working = rest;
                    false
                }
                None => true,
            },
        )
        .collect()
}
