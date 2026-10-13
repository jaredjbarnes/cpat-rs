//! AutoComplete (v12 `src/intellisense`): what could come next in a text,
//! from a recording parse and the introspection queries of milestone 8.
//! DESIGN §17.
//!
//! ```
//! use cpat_core::{AutoComplete, AutoCompleteOptions, Choice, Literal, Pattern, Sequence};
//!
//! let name: Pattern = Sequence::new("name", vec![
//!     Literal::new("john", "John").unwrap().into(),
//!     Literal::new("space", " ").unwrap().into(),
//!     Choice::new("last-name", vec![
//!         Literal::new("smith", "Smith").unwrap().into(),
//!         Literal::new("doe", "Doe").unwrap().into(),
//!     ]).unwrap().into(),
//! ]).unwrap().into();
//!
//! let mut auto_complete = AutoComplete::new(name, AutoCompleteOptions::default()).unwrap();
//! let suggestion = auto_complete.suggest_for("John ");
//! let texts: Vec<&str> = suggestion.options.iter().map(|o| o.text.as_str()).collect();
//! assert_eq!(texts, ["Doe", "Smith"]);
//! assert_eq!(suggestion.error_at_index, Some(5));
//! ```

mod auto_complete;
mod collate;
mod collate_latin;
mod collate_nfd;
mod composite;
mod options;
mod suggestion;
mod trim;

pub use auto_complete::{AutoComplete, AutoCompleteOptions};
pub use collate::locale_compare;
pub use suggestion::{SegmentPattern, Suggestion, SuggestionOption, SuggestionSegment};
