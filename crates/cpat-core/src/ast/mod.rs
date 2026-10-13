//! Abstract syntax trees beyond reading: v12's full `Node` API.
//!
//! A parse gives a read-only `Tree` (§7a). `Tree::to_mut` copies it into an
//! `EditableTree`, where nodes can be moved, replaced, compacted, transformed
//! and renumbered; nodes made with `create_node` live there too. Both forms
//! share the walks and print the same JSON.

mod by_name;
mod cycle_free;
mod editable;
mod equality;
mod from_tree;
mod json;
mod read_only;
mod rewrite;
mod search;
mod splice;
mod transform;
mod view;
mod walk;

pub use by_name::{compact, remove};
pub use cycle_free::CycleFreeNode;
pub use editable::{EditableTree, NodeRef};
pub use transform::Visitors;
