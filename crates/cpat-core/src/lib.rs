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

mod ancestors;
mod children;
mod cursor;
mod exec;
mod graphemes;
mod halt;
mod history;
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
mod tree;

pub use ancestors::Frame;
pub use cursor::Cursor;
pub use exec::ParseResult;
pub use halt::{DEFAULT_MAX_DEPTH, Limits, TooDeep};
pub use history::HistoryRecord;
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
pub use tree::{Node, NodeId, NodeKind, Tree};
