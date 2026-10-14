//! Rewrites a JavaScript regex source (as written in `.cpat` files, compiled
//! by v12 with the `u` flag) into Rust regex syntax with the same meaning,
//! or says why it can't (DESIGN §6).
//!
//! The source is parsed as JS reads it (`parser`), so what JS rejects is
//! rejected even where Rust would accept it, then written back out with
//! every JS-specific meaning spelled out (`emit`): `.`, `\s`, `\d`, `\w`,
//! `\b`, `\cX`, `\P{…}` under `i`, escaped surrogate pairs, empty classes,
//! modifier groups and backreferences to groups that haven't matched.

mod ast;
mod backreferences;
mod char_sets;
mod emit;
mod engine;
mod error;
mod folded_complement;
mod js_property_names;
mod mode;
mod parser;

pub(crate) use error::TranslateError;
pub(crate) use parser::MAX_DEPTH as MAX_GROUP_DEPTH;

/// Which engine a translation is for. They spell an ASCII word boundary
/// differently: `regex` switches Unicode off for it (`(?-u:\b)`), which
/// `fancy-regex` refuses, so for it the boundary is lookaround over the
/// ASCII word class.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Dialect {
    Fast,
    Fancy,
}

/// A source in Rust syntax, and the engine it needs.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Translation {
    pub rust: String,
    pub dialect: Dialect,
}

pub(crate) fn js_to_rust(source: &str) -> Result<Translation, TranslateError> {
    let root = parser::parse(source)?;
    backreferences::check(&root)?;
    let dialect = engine::dialect(&root);
    Ok(Translation {
        rust: emit::emit(&root, dialect),
        dialect,
    })
}

/// Whether the source ends with an unescaped `$`.
pub(crate) fn ends_with_anchor(source: &str) -> bool {
    let Some(before) = source.strip_suffix('$') else {
        return false;
    };
    let backslashes = before.chars().rev().take_while(|&c| c == '\\').count();
    backslashes % 2 == 0
}

#[cfg(test)]
mod tests;
