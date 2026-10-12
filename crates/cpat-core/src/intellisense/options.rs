//! Turning composite suggestions into placed options (v12
//! `_createSuggestions`, `findMatchIndex`, `getFurthestOptions`, and the
//! dedupe both share).

use std::collections::HashSet;

use super::collate::locale_compare;
use super::suggestion::{Composite, Segmented, SuggestionOption};

/// Places each composite: `typed` (the text up to where the suggestion
/// applies) plus the composite is compared with the whole `text`, and the
/// option starts where they part. Only the options that start furthest
/// are kept, sorted by their text as JS `localeCompare` sorts.
pub(super) fn place(typed: &str, text: &str, composites: Vec<Composite>) -> Vec<SuggestionOption> {
    let placed = composites.into_iter().map(|composite| {
        let candidate = format!("{typed}{}", composite.text);
        let start_index = shared_prefix(&candidate, text);
        SuggestionOption {
            text: candidate[start_index..].to_string(),
            start_index,
            suggestion_sequence: composite.sequence,
        }
    });
    let mut furthest = furthest(placed);
    // Stable, as JS's sort is: options that collate equal keep their order.
    furthest.sort_by(|a, b| locale_compare(&a.text, &b.text));
    furthest
}

/// v12 `findMatchIndex`: how far `a` and `b` agree, as a byte offset. v12
/// compares UTF-16 units; whole `char`s are compared here, so two texts
/// never part inside a character (DESIGN §17).
fn shared_prefix(a: &str, b: &str) -> usize {
    a.char_indices()
        .zip(b.chars())
        .find(|((_, x), y)| x != y)
        .map_or_else(|| a.len().min(b.len()), |((index, _), _)| index)
}

/// v12 `getFurthestOptions`: the options with the greatest start, in order.
pub(super) fn furthest(
    options: impl IntoIterator<Item = SuggestionOption>,
) -> Vec<SuggestionOption> {
    let mut kept: Vec<SuggestionOption> = Vec::new();
    for option in options {
        match kept.first().map(|first| first.start_index) {
            Some(start) if option.start_index < start => {}
            Some(start) if option.start_index == start => kept.push(option),
            _ => kept = vec![option],
        }
    }
    kept
}

/// Whether suggestions that share a text and the same segments are kept
/// once (v12's default) or every time (`disableDedupe`). Decided once, when
/// AutoComplete is built.
#[derive(Clone, Copy, Debug)]
pub(super) enum Dedupe {
    ByKey,
    Off,
}

impl Dedupe {
    pub(super) fn apply<T: Segmented>(self, suggestions: Vec<T>) -> Vec<T> {
        match self {
            Dedupe::Off => suggestions,
            Dedupe::ByKey => {
                let mut seen = HashSet::new();
                suggestions
                    .into_iter()
                    .filter(|suggestion| seen.insert(key(suggestion)))
                    .collect()
            }
        }
    }
}

/// v12's key, spaces included: the text, then each segment as
/// ` name - text  `, sorted by UTF-16 units (JS's default sort), joined
/// with `|`. Segments are
/// compared by name, not by instance, so two patterns with one name
/// offering one token collapse into one.
fn key(suggestion: &impl Segmented) -> String {
    let mut segments: Vec<String> = suggestion
        .segments()
        .iter()
        .map(|segment| format!(" {} - {}  ", segment.pattern.name, segment.text))
        .collect();
    segments.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    format!("{}|{}", suggestion.text(), segments.join("|"))
}

/// Keeps the first option with each text (v12 `_getAllSuggestionsOptions`
/// merges the options from matches and errors this way).
pub(super) fn first_per_text(options: Vec<SuggestionOption>) -> Vec<SuggestionOption> {
    let mut seen = HashSet::new();
    options
        .into_iter()
        .filter(|option| seen.insert(option.text.clone()))
        .collect()
}
