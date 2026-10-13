//! v12's `toJson(space)`: `JSON.stringify(node.toCycleFreeObject(), null,
//! space)`, written straight from the tree with a stack (§7e), so no nested
//! object is built and no walk recurses.

use std::fmt::Write;

use super::view::{AstView, joined_value};

/// `JSON.stringify` caps its indent at ten spaces.
const MAX_INDENT: usize = 10;

pub(crate) fn to_json<V: AstView>(view: &V, root: V::Ref, space: usize) -> String {
    let mut out = JsonOut {
        text: String::new(),
        indent: space.min(MAX_INDENT),
    };
    write_tree(view, root, &mut out);
    out.text
}

fn write_tree<V: AstView>(view: &V, root: V::Ref, out: &mut JsonOut) {
    // Each frame is a node whose `children` array is open, and how many of
    // its children have been written.
    let mut frames: Vec<(V::Ref, usize)> = Vec::new();
    open_node(view, root, 0, out);
    if !view.children_of(root).is_empty() {
        frames.push((root, 0));
    }

    loop {
        // A node at level d is an object at depth 2d: its keys sit at 2d + 1
        // and its children's objects at 2d + 2.
        let depth = frames.len() * 2;
        let Some(&mut (node, ref mut written)) = frames.last_mut() else {
            return;
        };
        let children = view.children_of(node);
        if *written < children.len() {
            let child = children[*written];
            if *written > 0 {
                out.text.push(',');
            }
            *written += 1;
            out.newline(depth);
            open_node(view, child, depth, out);
            if !view.children_of(child).is_empty() {
                frames.push((child, 0));
            }
            continue;
        }

        frames.pop();
        out.newline(depth - 1);
        out.text.push(']');
        out.newline(depth - 2);
        out.text.push('}');
    }
}

/// Writes a node's keys up to its `children` array, closing both when it has
/// no children (v12 prints those as `[]`).
fn open_node<V: AstView>(view: &V, node: V::Ref, depth: usize, out: &mut JsonOut) {
    out.text.push('{');
    out.key("id", depth + 1, true);
    out.string(&view.id_of(node).to_string());
    out.key("type", depth + 1, false);
    out.string(view.type_of(node));
    out.key("name", depth + 1, false);
    out.string(view.name_of(node));
    out.key("value", depth + 1, false);
    out.string(&joined_value(view, node));
    out.key("startIndex", depth + 1, false);
    let _ = write!(out.text, "{}", view.start_of(node));
    out.key("endIndex", depth + 1, false);
    let _ = write!(out.text, "{}", view.end_of(node));
    out.key("children", depth + 1, false);
    out.text.push('[');

    if view.children_of(node).is_empty() {
        out.text.push(']');
        out.newline(depth);
        out.text.push('}');
    }
}

struct JsonOut {
    text: String,
    indent: usize,
}

impl JsonOut {
    /// A line break and `depth` levels of indent, or nothing when compact.
    fn newline(&mut self, depth: usize) {
        if self.indent > 0 {
            self.text.push('\n');
            self.text
                .extend(std::iter::repeat_n(' ', self.indent * depth));
        }
    }

    fn key(&mut self, key: &str, depth: usize, first: bool) {
        if !first {
            self.text.push(',');
        }
        self.newline(depth);
        self.string(key);
        self.text.push(':');
        if self.indent > 0 {
            self.text.push(' ');
        }
    }

    /// `JSON.stringify`'s escaping: quotes, backslashes and control
    /// characters only, with the short forms where JSON has one and
    /// lowercase `\u00xx` otherwise. Everything else, `/` and U+2028
    /// included, goes out as it is.
    fn string(&mut self, value: &str) {
        self.text.push('"');
        for c in value.chars() {
            match c {
                '"' => self.text.push_str("\\\""),
                '\\' => self.text.push_str("\\\\"),
                '\u{8}' => self.text.push_str("\\b"),
                '\u{c}' => self.text.push_str("\\f"),
                '\n' => self.text.push_str("\\n"),
                '\r' => self.text.push_str("\\r"),
                '\t' => self.text.push_str("\\t"),
                c if c < ' ' => {
                    let _ = write!(self.text, "\\u{:04x}", c as u32);
                }
                c => self.text.push(c),
            }
        }
        self.text.push('"');
    }
}
