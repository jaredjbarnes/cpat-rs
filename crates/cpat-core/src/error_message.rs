use crate::exec::ParseResult;
use crate::introspect::Introspection;
use crate::pattern::Pattern;

/// v12 `generateErrorMessage`, character for character:
/// `Error at line L, column C. Hint: t1,t2`.
///
/// The hint is what may follow the furthest match (its pattern's next
/// tokens, from `pattern`, the root that `result` was parsed with), or,
/// when nothing matched, what can start `pattern`. Suggestions are trimmed
/// and blanks dropped.
///
/// Kept exactly as v12 prints it, quirks included, since callers match on
/// it:
/// - Nothing matched: the hint joins with `", "`. After a match it is a JS
///   array in a template string, which joins with `","`.
/// - Line and column count UTF-16 units, and the text measured runs one
///   unit PAST the match (v12's `cursor.substring` is inclusive of its end
///   index). So the column is the match's end column plus one, a newline
///   right after the match starts a new line at column 0, and an emoji
///   right after it counts one unit (half of it).
pub fn generate_error_message(pattern: &Pattern, result: &ParseResult<'_>) -> String {
    let leaf = result.leaf_match.as_ref().filter(|leaf| leaf.end > 0);
    let Some(leaf) = leaf else {
        let hint = clean(pattern.tokens()).join(", ");
        return format!("Error at line 1, column 1. Hint: {hint}");
    };

    // The leaf's path is as deep as the parse's limits allowed.
    let introspection = Introspection::with_limits(pattern, result.limits());
    let hint = clean(introspection.next_tokens(&leaf.path)).join(",");
    let (line, column) = line_and_column(result.text(), leaf.end);
    format!("Error at line {line}, column {column}. Hint: {hint}")
}

fn clean(suggestions: Vec<String>) -> Vec<String> {
    suggestions
        .iter()
        .map(|suggestion| suggestion.trim())
        .filter(|suggestion| !suggestion.is_empty())
        .map(str::to_string)
        .collect()
}

/// v12's line and column for a match ending at byte `end`: the UTF-16 text
/// up to one unit past the end, split on `\n`; the line count and the last
/// line's length.
fn line_and_column(text: &str, end: usize) -> (usize, usize) {
    let units = text[..end].encode_utf16().count() + 1;
    let (mut line, mut column) = (1, 0);
    for unit in text.encode_utf16().take(units) {
        if unit == u16::from(b'\n') {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }
    (line, column)
}
