//! v12's `patterns` template: a grammar's rules by camelCase name.

use std::collections::HashMap;

use crate::pattern::Pattern;

use super::Grammar;
use super::camel_case::kebab_to_camel_case;
use super::error::GrammarError;
use super::options::GrammarOptions;

/// Builds `text` (as `Grammar::parse_string` does, with default options)
/// and keys each rule by its name in camelCase: `full-name` is `fullName`.
/// Two names that come out the same keep the later rule, as in v12.
/// `patterns!` formats the text first.
pub fn patterns(text: &str) -> Result<HashMap<String, Pattern>, GrammarError> {
    patterns_with(GrammarOptions::default())(text)
}

/// `patterns` with `options` (v12 `createPatternsTemplate`): a function
/// that builds every text it is given with them.
pub fn patterns_with(
    options: GrammarOptions,
) -> impl Fn(&str) -> Result<HashMap<String, Pattern>, GrammarError> {
    move |text| {
        let built = Grammar::parse_string(text, options.clone())?;
        Ok(built
            .into_iter()
            .map(|(name, pattern)| (kebab_to_camel_case(&name), pattern))
            .collect())
    }
}
