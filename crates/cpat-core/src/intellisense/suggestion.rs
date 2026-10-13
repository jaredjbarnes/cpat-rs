use crate::exec::ParseResult;
use crate::halt::TooDeep;
use crate::parse_error::ParseError;
use crate::pattern_error::PatternError;
use crate::pattern_id::PatternId;
use crate::pattern_path::PatternPath;
use crate::tree::Tree;

/// What AutoComplete found for a text (v12 `Suggestion`).
pub struct Suggestion<'t> {
    /// The parse matched the whole text. Options may still be offered: a
    /// complete text can often go on (`"kahn"` could become `"kahnnnnnn"`).
    pub is_complete: bool,
    /// What could be inserted, each at its own `start_index`.
    pub options: Vec<SuggestionOption>,
    /// Where the text stops making sense, spanning to its end; `None` when
    /// it is complete.
    pub error: Option<ParseError>,
    /// `error`'s start, the place to show the problem.
    pub error_at_index: Option<usize>,
    /// The recording parse the suggestion came from: what v12's cursor
    /// holds after `suggestFor` (errors, matches, history), and the tree.
    /// `None` for empty text, which v12 doesn't parse.
    pub parse: Option<ParseResult<'t>>,
}

impl<'t> Suggestion<'t> {
    /// What the parse matched, whole or not (v12 `ast`).
    pub fn tree(&self) -> Option<&Tree<'t>> {
        self.parse.as_ref().and_then(ParseResult::tree)
    }

    /// The parse met a name that resolves to nothing past what
    /// `AutoComplete::new` could check (or a custom pattern failed), and
    /// halted (see `ParseResult`).
    pub fn pattern_error(&self) -> Option<&PatternError> {
        self.parse
            .as_ref()
            .and_then(|parse| parse.pattern_error.as_ref())
    }

    /// The input nested deeper than the limits allow, and the parse halted.
    pub fn too_deep(&self) -> Option<TooDeep> {
        self.parse.as_ref().and_then(|parse| parse.too_deep)
    }
}

/// One thing to insert (v12 `SuggestionOption`): `text` goes at
/// `start_index` (a byte offset), replacing whatever the text has from
/// there on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuggestionOption {
    pub text: String,
    pub start_index: usize,
    /// The tokens this option is made of, in order, each with the pattern
    /// it came from. A segment the text already completed is left out, and
    /// one the text started keeps its full token.
    pub suggestion_sequence: Vec<SuggestionSegment>,
}

/// A token of an option and the pattern it came from (v12
/// `SuggestionSegment`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuggestionSegment {
    pub text: String,
    pub pattern: SegmentPattern,
}

/// The pattern instance a segment came from. Patterns have no parents to
/// point back through (DESIGN §2), so it is named by its place in the root
/// AutoComplete was built with, which `Introspection::at` answers for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SegmentPattern {
    pub path: PatternPath,
    /// The name the instance answers to (a reference's target is named
    /// after the reference).
    pub name: &'static str,
    /// Shared by the instance's clones; `None` for a choice's operator.
    pub id: Option<PatternId>,
}

/// A suggestion before it has a place in the text (v12
/// `CompositeSuggestion`): the full text and the tokens it is made of.
#[derive(Clone, Debug)]
pub(super) struct Composite {
    pub(super) text: String,
    pub(super) sequence: Vec<SuggestionSegment>,
}

impl Composite {
    pub(super) fn single(text: String, pattern: SegmentPattern) -> Composite {
        Composite {
            sequence: vec![SuggestionSegment {
                text: text.clone(),
                pattern,
            }],
            text,
        }
    }
}

/// The options shared by composites and placed options: v12 dedupes both
/// with one generic function.
pub(super) trait Segmented {
    fn text(&self) -> &str;
    fn segments(&self) -> &[SuggestionSegment];
}

impl Segmented for Composite {
    fn text(&self) -> &str {
        &self.text
    }
    fn segments(&self) -> &[SuggestionSegment] {
        &self.sequence
    }
}

impl Segmented for SuggestionOption {
    fn text(&self) -> &str {
        &self.text
    }
    fn segments(&self) -> &[SuggestionSegment] {
        &self.suggestion_sequence
    }
}
