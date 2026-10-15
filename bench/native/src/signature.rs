//! What a workload's results must agree on before it is timed, in the
//! shape `bench/lib/signature.mjs` writes. Positions are UTF-16 units, as
//! v12 reports them, so byte offsets are converted here.

use cpat_core::{GrammarError, ParseResult, Patterns, Suggestion};
use serde_json::{Value, json};

/// FNV-1a over UTF-16 code units, as `hash` in signature.mjs.
pub fn hash(text: &str) -> u32 {
    text.encode_utf16().fold(0x811c_9dc5, |h, unit| {
        (h ^ u32::from(unit)).wrapping_mul(0x0100_0193)
    })
}

/// UTF-16 length of `text[..byte]`.
fn units(text: &str, byte: usize) -> usize {
    text[..byte.min(text.len())].encode_utf16().count()
}

pub fn build(built: &Result<Patterns, GrammarError>) -> Value {
    match built {
        Ok(patterns) => {
            let names: Vec<&str> = patterns.names().collect();
            json!({ "rules": names.len(), "names": hash(&names.join("\n")) })
        }
        // v12 throws an `Error` whose message is the grammar error.
        Err(error) => json!({ "threw": format!("Error: {error}") }),
    }
}

/// A match is `[1, nodes, root length, root hash]`; a miss
/// `[0, error start, error last]`.
pub fn exec(result: &ParseResult<'_>, text: &str) -> Value {
    if let Some(error) = &result.pattern_error {
        return json!([format!("threw Error: {error}")]);
    }
    if result.too_deep.is_some() {
        return json!(["tooDeep"]);
    }
    let Some(tree) = &result.tree else {
        return match &result.error {
            Some(error) => json!([0, units(text, error.start), units(text, error.last)]),
            None => json!([0, -1, -1]),
        };
    };
    let mut nodes = 0usize;
    let mut stack = vec![tree.root()];
    while let Some(node) = stack.pop() {
        nodes += 1;
        stack.extend(node.children());
    }
    let value = tree.root().value();
    json!([1, nodes, value.encode_utf16().count(), hash(value)])
}

/// `[complete, error at, option count, options hash]`.
pub fn suggest(suggestion: &Suggestion<'_>, text: &str) -> Value {
    if suggestion.too_deep().is_some() {
        return json!(["tooDeep"]);
    }
    let options: Vec<String> = suggestion
        .options
        .iter()
        .map(|o| format!("{}\u{0}{}", o.text, units(text, o.start_index)))
        .collect();
    let at = suggestion
        .error_at_index
        .map_or(-1, |at| units(text, at) as i64);
    json!([
        u8::from(suggestion.is_complete),
        at,
        options.len(),
        hash(&options.join("\u{1}"))
    ])
}

#[cfg(test)]
mod tests {
    use super::hash;

    #[test]
    fn hashes_utf16_units_as_the_js_side_does() {
        // FNV-1a of "" is the offset basis; "a" and a surrogate pair are
        // values the JS `hash` gives for the same strings.
        assert_eq!(hash(""), 0x811c_9dc5);
        assert_eq!(hash("a"), 0xe40c_292c);
        assert_eq!(hash("😀"), 0xcb31_c4b8);
    }
}
