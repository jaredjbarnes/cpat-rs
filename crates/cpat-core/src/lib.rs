//! cpat-core: the clarity pattern parser in Rust.
//!
//! Small composable patterns (`Literal`, `Regex`, `Sequence`, `Choice`,
//! `Optional`, `Not`, `Cut`) build parsers; `exec` runs one over a text and
//! returns a read-only tree whose values are slices of that text.
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

mod cursor;
mod exec;
mod graphemes;
mod name;
mod parse_error;
mod pattern;
mod pattern_error;
mod pattern_id;
mod patterns;
mod regex_translate;
mod tree;

pub use cursor::Cursor;
pub use exec::ParseResult;
pub use name::Name;
pub use parse_error::{Match, ParseError};
pub use pattern::Pattern;
pub use pattern_error::PatternError;
pub use pattern_id::{Meta, PatternId};
pub use patterns::{Choice, Custom, CustomParse, Cut, Literal, Not, Optional, Regex, Sequence};
pub use tree::{Node, NodeId, NodeKind, Tree};
