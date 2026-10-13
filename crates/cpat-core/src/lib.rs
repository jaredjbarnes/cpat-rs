//! cpat-core: the clarity pattern parser in Rust.
//!
//! Small composable patterns (`Literal`, `Regex`, `Sequence`, `Choice`,
//! `Repeat`, `Optional`, `Not`, `Cut`, `Reference`, `Context`,
//! `RightAssociated`, `TakeUntil`, `Block`) build
//! parsers; `exec` runs one over a text and returns a read-only tree whose
//! values are slices of that text.
//!
//! ```
//! use cpat_core::{Literal, Pattern, Regex, Sequence};
//!
//! let mut full_name: Pattern = Sequence::new("full-name", vec![
//!     Literal::new("first-name", "John").unwrap().into(),
//!     Regex::new("space", r"\s+").unwrap().into(),
//!     Literal::new("last-name", "Doe").unwrap().into(),
//! ]).unwrap().into();
//!
//! let result = full_name.exec("John Doe");
//! assert_eq!(result.tree().unwrap().root().value(), "John Doe");
//! ```

/// A grammar's rules by camelCase name (`patterns`), from text that may be
/// formatted: `patterns!(text)`, or `patterns!("x = {}", y)` as `format!`
/// takes it. With one argument the text is used as it is, so a grammar's
/// own braces (`with params { … }`, `["{"]`) need no escaping.
///
/// ```
/// let patterns = cpat_core::patterns!(r#"
///     first-name = "John"
///     full-name = first-name + " Doe"
/// "#).unwrap();
/// let mut full_name = patterns["fullName"].clone();
/// assert!(full_name.test("John Doe"));
/// ```
#[macro_export]
macro_rules! patterns {
    ($text:expr $(,)?) => {
        $crate::patterns($text)
    };
    ($format:literal, $($args:tt)+) => {
        $crate::patterns(&::std::format!($format, $($args)+))
    };
}

mod ancestors;
mod children;
mod cursor;
mod equality;
mod error_message;
mod exec;
mod grammar;
mod graphemes;
mod halt;
mod history;
mod intellisense;
mod introspect;
mod leaves;
mod link;
mod name;
mod parse_error;
#[cfg(feature = "parse-log")]
pub mod parse_log;
mod pattern;
mod pattern_error;
mod pattern_id;
mod pattern_path;
mod patterns;
mod regex_translate;
mod scope;
mod tokenizable;
mod tree;

pub use ancestors::Frame;
pub use cursor::Cursor;
pub use error_message::generate_error_message;
pub use exec::ParseResult;
pub use grammar::{
    Decorator, Decorators, Grammar, GrammarError, GrammarFile, GrammarOptions, ImportResolver,
    InMemoryImports, Patterns, cpat_grammar, patterns, patterns_with,
};
pub use halt::{DEFAULT_MAX_DEPTH, Limits, TooDeep};
pub use history::HistoryRecord;
pub use intellisense::{
    AutoComplete, AutoCompleteOptions, SegmentPattern, Suggestion, SuggestionOption,
    SuggestionSegment, locale_compare,
};
pub use introspect::{Handle, Introspection};
pub use name::Name;
pub use parse_error::{Match, ParseError};
pub use pattern::Pattern;
pub use pattern_error::PatternError;
pub use pattern_id::{Meta, PatternId};
pub use pattern_path::PatternPath;
pub use patterns::{
    Block, Choice, ChoiceMode, Context, Custom, CustomParse, Cut, Literal, Not, Optional,
    Reference, Regex, Repeat, RepeatOptions, RightAssociated, Sequence, TakeUntil,
};
/// The JSON a decorator's argument is (`@name(json)`).
pub use serde_json::Value as Json;
pub use tokenizable::Tokenizable;
pub use tree::{Node, NodeId, NodeKind, Tree};
