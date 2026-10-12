use crate::cursor::Cursor;
use crate::pattern::Pattern;
use crate::pattern_error::PatternError;
use crate::pattern_id::Meta;
use crate::tree::NodeId;

/// `a | b | c`: the first alternative that matches wins.
///
/// This is the ordered mode of the one choice pattern. Operator alternatives
/// (prefix, postfix, infix) will get their own mode on this same type, chosen
/// once when it is built, so plain choices keep this tight loop.
#[derive(Clone, Debug)]
pub struct Choice {
    pub(crate) meta: Meta,
    pub(crate) alternatives: Vec<Pattern>,
}

impl Choice {
    pub fn new(name: &str, alternatives: Vec<Pattern>) -> Result<Choice, PatternError> {
        if alternatives.is_empty() {
            return Err(PatternError::NoChildren {
                name: name.to_string(),
                kind: "choice",
            });
        }

        Ok(Choice {
            meta: Meta::new(name),
            alternatives,
        })
    }

    pub fn alternatives(&self) -> &[Pattern] {
        &self.alternatives
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();
        let alternatives = &mut self.alternatives;

        let (node, committed) = cursor.in_choice_scope(|cursor| {
            for alternative in alternatives.iter_mut() {
                cursor.set_pos(first);

                if let Some(node) = alternative.parse(cursor) {
                    return (Some(node), false);
                }

                // A cut fired inside the alternative that just failed: the
                // choice is committed to it, so no sibling gets a turn.
                if cursor.cut() {
                    return (None, true);
                }

                cursor.resolve_error();
            }

            (None, false)
        });

        if let Some(node) = node {
            cursor.resolve_error();
            return Some(node);
        }

        cursor.set_pos(first);

        // A committed failure keeps the failed alternative's deep error: that
        // branch's diagnosis is the one the grammar author meant, not a
        // shallow "no alternative matched here".
        if !committed || !cursor.has_error() {
            cursor.record_error(first, first, self.meta);
        }

        None
    }
}
