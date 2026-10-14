//! Whether a parse's root is the whole text, as v12's `exec` decides it:
//! `ast.value === text`, where a node's value is its leaves' values joined.
//!
//! A leaf can end inside a grapheme: a literal `"~"` matches the first code
//! point of `~́` (a tilde and a combining accent), and the next pattern
//! starts after the whole grapheme, so the accent belongs to no node. The
//! root then spans the text but its value leaves the accent out, and v12
//! reports no match. Comparing the root's end with the text's would accept
//! it.

use crate::cursor::Cursor;
use crate::tree::{NodeId, Tree};

/// What `root_value` reads of a tree: a parse still running (`exec` asks
/// its cursor) or a finished one (AutoComplete asks the tree it kept). One
/// rule for both, so `exec` and `suggest_for` agree on what is complete.
pub(crate) trait LeafSpans {
    fn text(&self) -> &str;
    fn span(&self, node: NodeId) -> (usize, usize);
    fn children(&self, node: NodeId) -> &[NodeId];
}

impl LeafSpans for Cursor<'_> {
    fn text(&self) -> &str {
        Cursor::text(self)
    }
    fn span(&self, node: NodeId) -> (usize, usize) {
        (self.node_start(node), self.node_end(node))
    }
    fn children(&self, node: NodeId) -> &[NodeId] {
        self.node_children(node)
    }
}

impl LeafSpans for Tree<'_> {
    fn text(&self) -> &str {
        Tree::text(self)
    }
    fn span(&self, node: NodeId) -> (usize, usize) {
        let node = self.node(node);
        (node.start(), node.end())
    }
    fn children(&self, node: NodeId) -> &[NodeId] {
        self.child_ids(node)
    }
}

/// The root's value measured against the text: whether it is the text, and
/// its length in UTF-16 units (v12's `ast.value.length`).
pub(crate) struct RootValue {
    pub is_text: bool,
    pub utf16_len: usize,
}

/// Walks the leaves in order (iteratively: an operator chain makes a tree as
/// deep as the input is long). They are the text exactly when each starts
/// where the one before ended and the last ends at the end of the text.
pub(crate) fn root_value(tree: &impl LeafSpans, root: NodeId) -> RootValue {
    let text = tree.text();
    let mut at = 0;
    let mut tiles = true;
    let mut utf16_len = 0;
    let mut stack = vec![root];
    while let Some(node) = stack.pop() {
        let children = tree.children(node);
        if children.is_empty() {
            let (start, end) = tree.span(node);
            tiles &= start == at;
            at = end;
            utf16_len += text[start..end].encode_utf16().count();
        } else {
            stack.extend(children.iter().rev());
        }
    }
    RootValue {
        is_text: tiles && at == text.len(),
        utf16_len,
    }
}

/// The byte offset of UTF-16 unit `units` in `text`, or of the character
/// that unit falls inside (the second half of a surrogate pair), or the
/// end. v12 records the error after a partial match at the matched value's
/// length, a count of UTF-16 units used as a position.
pub(crate) fn byte_at_utf16(text: &str, units: usize) -> usize {
    let mut counted = 0;
    for (byte, c) in text.char_indices() {
        let next = counted + c.len_utf16();
        if next > units {
            return byte;
        }
        counted = next;
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::byte_at_utf16;

    #[test]
    fn utf16_units_map_to_the_character_they_fall_in() {
        let text = "a😀é";
        // a: unit 0, byte 0; 😀: units 1-2, bytes 1-4; é: unit 3, bytes 5-6.
        assert_eq!(
            [0, 1, 2, 3, 4, 9].map(|units| byte_at_utf16(text, units)),
            [0, 1, 1, 5, 7, 7]
        );
    }
}
