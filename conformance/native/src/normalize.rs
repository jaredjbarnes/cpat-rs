//! The normalized shape `conformance/lib/normalize.mjs` defines, built from
//! the core's results. Keys and encodings must stay the same as there.

use cpat_core::{ParseError, Tree};
use serde_json::{Value, json};

use crate::utf16::Utf16;

/// Pre-order rows `depth|type|name|start|end`, plus a `value!` row after a
/// node whose value isn't the text it spans.
pub fn ast_rows(tree: Option<&Tree<'_>>, text: &str, offsets: &Utf16) -> Value {
    let Some(tree) = tree else {
        return Value::Null;
    };
    let mut rows = Vec::new();
    let mut stack = vec![(tree.root(), 0usize)];
    while let Some((node, depth)) = stack.pop() {
        rows.push(format!(
            "{depth}|{}|{}|{}|{}",
            node.kind().as_str(),
            node.name(),
            offsets.of(node.start()),
            offsets.of(node.end())
        ));
        if node.value() != &text[node.start()..node.end()] {
            rows.push(format!("value!{}", Value::from(node.value())));
        }
        stack.extend(node.children().rev().map(|child| (child, depth + 1)));
    }
    Value::from(rows.join("\n"))
}

/// `[start, last, name]`, `last` inclusive.
pub fn error_triple(error: Option<&ParseError>, offsets: &Utf16) -> Value {
    match error {
        None => Value::Null,
        Some(error) => json!([
            offsets.of(error.start),
            offsets.of(error.last),
            error.pattern.name.as_str()
        ]),
    }
}
