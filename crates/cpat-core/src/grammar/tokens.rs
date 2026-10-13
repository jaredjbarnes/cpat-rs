//! `@tokens([...])`, the built-in decorator (v12 `decorators/tokens.ts`):
//! the tokens a pattern offers to autocomplete.

use serde_json::Value;

use crate::pattern::Pattern;

/// Hands `arg`, an array of strings, to the pattern's `set_tokens`. The
/// target is found by capability (`Pattern::tokenizable_mut`), so a regex, a
/// take-until and a custom pattern all take it. Misuse is an error, never a
/// silent no-op.
pub(super) fn tokens(mut pattern: Pattern, arg: Option<&Value>) -> Result<Pattern, String> {
    let Some(tokens) = string_array(arg) else {
        return Err(format!(
            "`@tokens` on `{}` expects an array of strings",
            pattern.name()
        ));
    };

    let name = pattern.name();
    let type_name = pattern.type_name();
    let Some(target) = pattern.tokenizable_mut() else {
        return Err(format!(
            "`@tokens` can't apply to `{name}` (a {type_name}) — only patterns with setTokens \
             accept tokens"
        ));
    };
    target.set_tokens(tokens);
    Ok(pattern)
}

fn string_array(arg: Option<&Value>) -> Option<Vec<String>> {
    match arg? {
        Value::Array(items) => items
            .iter()
            .map(|item| item.as_str().map(str::to_string))
            .collect(),
        _ => None,
    }
}
