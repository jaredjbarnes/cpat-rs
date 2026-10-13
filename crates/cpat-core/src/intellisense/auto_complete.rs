use std::collections::HashMap;

use crate::exec::{ParseResult, parse_recording};
use crate::halt::Limits;
use crate::introspect::{Handle, Introspection};
use crate::parse_error::{Match, ParseError};
use crate::pattern::Pattern;
use crate::pattern_error::PatternError;
use crate::pattern_path::PatternPath;

use super::composite::Composer;
use super::options::{Dedupe, first_per_text, furthest, place};
use super::suggestion::{Suggestion, SuggestionOption};
use super::trim::trim_by_existing_text;

/// How AutoComplete builds its suggestions (v12 `AutoCompleteOptions`).
#[derive(Clone, Debug, Default)]
pub struct AutoCompleteOptions {
    /// Patterns whose suggestions are joined with whatever may follow them
    /// (a divider or a space offered together with the next word). Every
    /// suggestion times every next suggestion, so keep these few.
    pub greedy_pattern_names: Vec<String>,
    /// Extra suggestions for the patterns with these names, before their
    /// own tokens (free text, for example).
    pub custom_tokens: HashMap<String, Vec<String>>,
    /// Keep every suggestion, even ones with the same text and the same
    /// segments by name (two patterns with one name offering one token).
    pub disable_dedupe: bool,
}

/// Suggests what could come next in a text (v12 `AutoComplete`).
///
/// The text is parsed once with a recording cursor, and the suggestions
/// come from the furthest matches (what could follow each, and what
/// could still extend it) and from the patterns that failed because the
/// text ran out. All positions are byte offsets into the text.
pub struct AutoComplete {
    pattern: Pattern,
    greedy_pattern_names: Vec<String>,
    custom_tokens: HashMap<String, Vec<String>>,
    dedupe: Dedupe,
    limits: Limits,
}

impl AutoComplete {
    /// Fails when the pattern names a reference that resolves to nothing
    /// (DESIGN §11): it could never suggest past it. v12 throws once a
    /// parse or a query reaches the name.
    pub fn new(
        pattern: Pattern,
        options: AutoCompleteOptions,
    ) -> Result<AutoComplete, PatternError> {
        AutoComplete::with_limits(pattern, options, Limits::default())
    }

    /// `new`, parsing under `limits` (see `Pattern::exec_with`).
    pub fn with_limits(
        pattern: Pattern,
        options: AutoCompleteOptions,
        limits: Limits,
    ) -> Result<AutoComplete, PatternError> {
        pattern.check()?;
        Ok(AutoComplete {
            pattern,
            greedy_pattern_names: options.greedy_pattern_names,
            custom_tokens: options.custom_tokens,
            dedupe: if options.disable_dedupe {
                Dedupe::Off
            } else {
                Dedupe::ByKey
            },
            limits,
        })
    }

    pub fn pattern(&self) -> &Pattern {
        &self.pattern
    }

    /// What could come next in `text`, and where it stops making sense.
    pub fn suggest_for<'t>(&mut self, text: &'t str) -> Suggestion<'t> {
        if text.is_empty() {
            return self.suggest_for_empty_text();
        }
        let result = parse_recording(&mut self.pattern, text, self.limits);
        let intro = Introspection::with_limits(&self.pattern, self.limits);
        let composer = self.composer(&intro);
        let suggester = Suggester {
            text,
            intro: &intro,
            composer: &composer,
        };
        let options = suggester.all_options(&result);
        let root_end = result
            .tree
            .as_ref()
            .map(|tree| (tree.root().start(), tree.root().end()));
        // v12 compares the ast's value with the text; a node's value is
        // the slice it spans (DESIGN §7a).
        let is_complete = root_end == Some((0, text.len()));
        let (error, error_at_index) = if is_complete {
            (None, None)
        } else {
            self.locate_error(&result, &options, root_end.map(|(_, end)| end))
        };

        Suggestion {
            is_complete,
            options,
            error,
            error_at_index,
            parse: Some(result),
        }
    }

    /// v12 doesn't parse empty text: it offers what can start the pattern,
    /// with an error at 0.
    fn suggest_for_empty_text(&self) -> Suggestion<'static> {
        let intro = Introspection::with_limits(&self.pattern, self.limits);
        let composer = self.composer(&intro);
        let options = match intro.at(&PatternPath::root()) {
            Some(root) => place("", "", composer.for_pattern(&root)),
            None => Vec::new(),
        };
        Suggestion {
            is_complete: false,
            options,
            error: Some(self.error_at(0, 0)),
            error_at_index: Some(0),
            parse: None,
        }
    }

    /// v12's three cases, in its order, for a text that isn't complete.
    fn locate_error(
        &self,
        result: &ParseResult<'_>,
        options: &[SuggestionOption],
        root_end: Option<usize>,
    ) -> (Option<ParseError>, Option<usize>) {
        let len = result.text().len();
        let has_error = result.error.is_some();
        if !options.is_empty() && !has_error {
            // The parse stopped cleanly short of the end: the error runs
            // from the earliest option to the end of the text.
            let start = options
                .iter()
                .map(|option| option.start_index)
                .min()
                .expect("options is not empty");
            return (Some(self.error_at(start, len)), Some(start));
        }
        if let (true, Some(end)) = (options.is_empty(), root_end) {
            // Matched a prefix and nothing could follow it.
            return (Some(self.error_at(end, len)), Some(end));
        }
        if let (true, Some(furthest_error)) = (has_error, &result.furthest_error) {
            let at = furthest_position(furthest_error, result.matches().last());
            return (Some(self.error_at(at, at)), Some(at));
        }
        (None, None)
    }

    /// v12 names the root in AutoComplete's own errors.
    fn error_at(&self, start: usize, last: usize) -> ParseError {
        ParseError {
            start,
            last,
            pattern: self.pattern.meta(),
            path: None,
        }
    }

    fn composer<'a, 'r>(&'a self, intro: &'a Introspection<'r>) -> Composer<'a, 'r> {
        Composer::new(
            intro,
            &self.greedy_pattern_names,
            &self.custom_tokens,
            self.dedupe,
        )
    }
}

/// v12 `getFurthestPosition`: the further of the last recorded match's end
/// and the furthest error's `last`. (v12 compares an exclusive end with an
/// inclusive last, and so does this.)
fn furthest_position(furthest_error: &ParseError, last_match: Option<&Match>) -> usize {
    match last_match {
        Some(found) if found.end > furthest_error.last => found.end,
        _ => furthest_error.last,
    }
}

/// One `suggest_for` over a parsed text.
struct Suggester<'a, 'r, 't> {
    text: &'t str,
    intro: &'a Introspection<'r>,
    composer: &'a Composer<'a, 'r>,
}

impl<'r> Suggester<'_, 'r, '_> {
    /// v12 `_getAllSuggestionsOptions`: the options from the furthest
    /// matches, then those from errors at the end of the text, the first
    /// with each text, and of those the ones that start furthest.
    ///
    /// v12 also dedupes each match's composites and the error options by
    /// text and segment names; that is left out, since keeping the first
    /// option per text here drops whatever it would (each composite
    /// pattern's own suggestions are still deduped, `Composer`).
    fn all_options(&self, result: &ParseResult<'_>) -> Vec<SuggestionOption> {
        let mut options: Vec<SuggestionOption> = match result.leaf_matches() {
            // v12's leaf matches start as one empty match, which offers
            // what can start the root.
            [] => self.options_from_root(),
            matches => matches.iter().flat_map(|m| self.options_after(m)).collect(),
        };
        options.extend(self.options_from_errors(result.errors()));
        furthest(first_per_text(options))
    }

    fn options_from_root(&self) -> Vec<SuggestionOption> {
        match self.intro.at(&PatternPath::root()) {
            Some(root) => place("", self.text, self.composer.for_pattern(&root)),
            None => Vec::new(),
        }
    }

    /// What may follow a match, and what could still extend it (a literal
    /// set or custom tokens offering longer texts that start the same).
    fn options_after(&self, found: &Match) -> Vec<SuggestionOption> {
        let Some(handle) = self.intro.at(&found.path) else {
            return Vec::new();
        };
        let current = &self.text[found.start..found.end];
        let extending = trim_by_existing_text(current, self.composer.for_pattern(&handle));
        let mut composites: Vec<_> = self
            .intro
            .next_patterns(&found.path)
            .iter()
            .flat_map(|next| self.composer.for_pattern(next))
            .collect();
        composites.extend(extending);
        place(&self.text[..found.end], self.text, composites)
    }

    /// v12 `_createSuggestionOptionsFromErrors`: the patterns that failed
    /// because the text ran out, completed from where they began.
    ///
    /// v12 keeps the errors whose `lastIndex` is `length - 1`, the last
    /// UTF-16 unit. Here that is `last` at the start of the last `char`:
    /// what a `Literal` that ran out of text records in both (DESIGN §17).
    fn options_from_errors(&self, errors: &[ParseError]) -> Vec<SuggestionOption> {
        let last_char = self.text.char_indices().last().map(|(index, _)| index);
        errors
            .iter()
            .filter(|error| Some(error.last) == last_char)
            .filter_map(|error| Some((error, self.handle(error.path.as_ref()?)?)))
            .flat_map(|(error, handle)| {
                let current = &self.text[error.start..];
                let composites = trim_by_existing_text(current, self.composer.for_pattern(&handle));
                // v12's `substring(0, lastIndex)` is inclusive: all of it.
                place(self.text, self.text, composites)
            })
            .collect()
    }

    fn handle(&self, path: &PatternPath) -> Option<Handle<'r>> {
        self.intro.at(path)
    }
}
