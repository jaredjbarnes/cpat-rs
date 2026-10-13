//! `\P{…}` under `i`, as JS means it.
//!
//! JS (`u` and `i`) takes the complement first and then matches every
//! character whose simple case fold equals that of a member. Rust's `(?i)`
//! does it the other way round: it folds the property, then negates. For a
//! property not closed under case folding the two disagree on case partners:
//! JS's `(?i:\P{Lu})` matches `A` (its partner `a` isn't `Lu`), Rust's
//! doesn't. So the complement is built and folded here, and written out as
//! ranges that mean the same to any engine and under any flags.

use regex_syntax::ParserBuilder;
use regex_syntax::hir::{Class, ClassUnicode, HirKind};

/// The inclusive code-point ranges of `\P{name}` closed under simple case
/// folding.
///
/// Panics if Rust doesn't know `name`: the parser only lets through names
/// that compile in Rust (`every_js_property_name_compiles_in_rust`).
pub(super) fn folded_complement(name: &str) -> Vec<(u32, u32)> {
    // The positive property, negated here: regex-syntax turns a class with
    // no members (`\P{Any}`) into a byte class that never matches.
    let hir = ParserBuilder::new()
        .build()
        .parse(&format!(r"\p{{{name}}}"))
        .expect("the parser admits only property names Rust knows");
    let mut class = match hir.into_kind() {
        HirKind::Class(Class::Unicode(class)) => class,
        _ => ClassUnicode::empty(),
    };
    class.negate();
    class.case_fold_simple();
    class
        .ranges()
        .iter()
        .map(|range| (range.start() as u32, range.end() as u32))
        .collect()
}
