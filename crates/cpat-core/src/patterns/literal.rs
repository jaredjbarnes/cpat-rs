use crate::cursor::Cursor;
use crate::pattern_error::PatternError;
use crate::pattern_id::Meta;
use crate::tree::{NodeId, NodeKind};

/// Matches an exact string.
#[derive(Clone, Debug)]
pub struct Literal {
    pub(crate) meta: Meta,
    token: Box<str>,
}

impl Literal {
    pub fn new(name: &str, token: &str) -> Result<Literal, PatternError> {
        if token.is_empty() {
            return Err(PatternError::EmptyLiteral {
                name: name.to_string(),
            });
        }

        Ok(Literal {
            meta: Meta::new(name),
            token: token.into(),
        })
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();

        if let Some(last) = mismatch(cursor.text(), first, &self.token) {
            cursor.record_error(first, last, self.meta);
            return None;
        }

        let end = first + self.token.len();
        cursor.resolve_error();
        let node = cursor.leaf(NodeKind::Literal, self.meta.name, first, end);
        cursor.record_match(self.meta, first, end);

        // A token can end inside a grapheme ("e" against "e" + combining
        // accent). The node covers exactly the token; the cursor continues at
        // the next boundary, as the TS library does.
        cursor.set_pos(cursor.snap_forward(end));
        Some(node)
    }
}

/// `None` when `token` occurs at `first`; otherwise the inclusive byte offset
/// the comparison reached: the first differing character, or the last
/// character of the text when it ends before the token does.
fn mismatch(text: &str, first: usize, token: &str) -> Option<usize> {
    let rest = &text[first..];

    for ((offset, expected), actual) in token.char_indices().zip(rest.chars()) {
        if expected != actual {
            return Some(first + offset);
        }
    }

    if rest.len() >= token.len() {
        return None;
    }

    // The text ran out. Point at its last character, or at the end itself
    // when there was nothing left to compare.
    Some(match rest.char_indices().last() {
        Some((offset, _)) => first + offset,
        None => text.len(),
    })
}
