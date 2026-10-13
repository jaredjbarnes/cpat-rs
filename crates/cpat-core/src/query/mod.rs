//! v12's query layer: CSS-like selectors over an `EditableTree` and a
//! jQuery-like `Query` that selects and edits through them.
//!
//! A selector is parsed with a grammar written in cpat itself
//! (`grammar.rs`), then compiled once into the steps v12 runs on every
//! call (`step.rs`), so running a selector never looks at its text again.

mod attribute;
mod combinator;
mod error;
mod grammar;
mod js;
mod node_query;
mod selector;
mod step;

pub use error::SelectorError;
pub use node_query::Query;
pub use selector::Selector;
