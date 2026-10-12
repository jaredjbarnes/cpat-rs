mod scan;

use crate::cursor::Cursor;
use crate::name::Name;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
use crate::tree::{NodeId, NodeKind};

use super::Literal;

/// A delimited region: `open`, then whatever lies before the `close` that
/// balances it, then that `close`. The matching close is found by counting
/// delimiters in the text, not by parsing, so the content is parsed only
/// within known bounds and must end exactly at the close.
///
/// The delimiters are literals, so the types enforce what v12's
/// constructor signature asks for: the scan searches for their tokens.
///
/// With no content pattern (a "wildcard" block) the text between the
/// delimiters becomes a `BlockContent` node named `<name>-content`.
#[derive(Clone, Debug)]
pub struct Block {
    pub(crate) meta: Meta,
    // Patterns, not `Literal`s, so a search through the grammar sees them
    // as v12's `children` (a reference named `open` can find one). Built
    // only from literals, so `open_token` and `close_token` are theirs.
    pub(crate) open: Box<Pattern>,
    pub(crate) content: Option<Box<Pattern>>,
    pub(crate) close: Box<Pattern>,
    open_token: Box<str>,
    close_token: Box<str>,
    // The wildcard content node's name, kept in step with the block's name
    // (`Pattern::rename`), so a parse doesn't format and intern it.
    content_name: Name,
    // Decided once: v12 lets content that didn't match leave the block
    // valid only when the content pattern is an `Optional`.
    content_may_be_absent: bool,
}

/// Why a block can't keep its content.
struct ContentFailed;

impl Block {
    pub fn new(name: &str, open: Literal, content: Option<Pattern>, close: Literal) -> Block {
        let content_may_be_absent = matches!(content, Some(Pattern::Optional(_)));
        Block {
            meta: Meta::new(name),
            open_token: open.token().into(),
            close_token: close.token().into(),
            open: Box::new(open.into()),
            content: content.map(|content| Box::new(content.adopted())),
            close: Box::new(close.into()),
            content_name: content_name(Name::new(name)),
            content_may_be_absent,
        }
    }

    pub fn open(&self) -> &Pattern {
        &self.open
    }

    pub fn content(&self) -> Option<&Pattern> {
        self.content.as_deref()
    }

    pub fn close(&self) -> &Pattern {
        &self.close
    }

    pub(crate) fn renamed_content(&mut self) {
        self.content_name = content_name(self.meta.name);
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();
        match self.parse_parts(cursor, first) {
            Some(node) => Some(node),
            None => {
                // v12 rewinds, then records at (first, where the cursor is),
                // which is `first` again.
                cursor.set_pos(first);
                cursor.record_error(first, first, self.meta);
                None
            }
        }
    }

    fn parse_parts(&mut self, cursor: &mut Cursor<'_>, first: usize) -> Option<NodeId> {
        // Phase 1: the open delimiter.
        let open = self.open.parse(cursor).filter(|_| !cursor.has_error())?;
        let open_end = cursor.node_end(open);

        // Phase 2: where the close that balances it starts.
        let close_start =
            scan::matching_close(cursor.text(), open_end, &self.open_token, &self.close_token)?;

        // Phase 3: the content, between the two.
        let content = self.parse_content(cursor, open_end, close_start).ok()?;

        // Phase 4: the close, where the scan found it. v12's `moveTo` lands
        // on the start of the grapheme holding that index; so does this.
        cursor.set_pos(cursor.snap_back(close_start));
        let close = self.close.parse(cursor).filter(|_| !cursor.has_error())?;
        let end = cursor.node_end(close);

        let inner = match content {
            Some(content) => Some(content),
            None if self.content.is_none() && open_end < close_start => Some(cursor.leaf(
                NodeKind::BlockContent,
                self.content_name,
                open_end,
                close_start,
            )),
            None => None,
        };
        let node = match inner {
            Some(inner) => cursor.branch(
                NodeKind::Block,
                self.meta.name,
                first,
                end,
                &[open, inner, close],
            ),
            None => cursor.branch(NodeKind::Block, self.meta.name, first, end, &[open, close]),
        };
        cursor.record_match(self.meta, first, end);
        Some(node)
    }

    /// The content node, if any. Adjacent delimiters leave no room for
    /// content, and that is always valid: it is the base case of a
    /// recursive block. Otherwise content that didn't match fails the block
    /// unless it is optional, and content that matched must end exactly
    /// where the close starts.
    fn parse_content(
        &mut self,
        cursor: &mut Cursor<'_>,
        open_end: usize,
        close_start: usize,
    ) -> Result<Option<NodeId>, ContentFailed> {
        let Some(content) = &mut self.content else {
            return Ok(None);
        };
        if open_end >= close_start {
            return Ok(None);
        }

        // An open token can end inside a grapheme; the content starts at the
        // next boundary, where the open literal left the cursor.
        cursor.set_pos(cursor.snap_forward(open_end));
        // v12 also checks `cursor.hasError` here. A failed pattern always
        // returns no node, and the only content allowed none is an
        // `Optional`, which never leaves an error, so the match says it all.
        match content.parse(cursor) {
            None if self.content_may_be_absent => Ok(None),
            None => Err(ContentFailed),
            Some(node) if cursor.node_end(node) != close_start => Err(ContentFailed),
            Some(node) => Ok(Some(node)),
        }
    }
}

fn content_name(block: Name) -> Name {
    Name::new(&format!("{block}-content"))
}
