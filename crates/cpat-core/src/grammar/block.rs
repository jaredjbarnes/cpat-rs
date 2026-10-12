//! `["{"] content ["}"]` and `["{"] ... ["}"]` (v12 `Grammar._buildBlock`).

use crate::pattern::Pattern;
use crate::patterns::{Block, Literal};
use crate::tree::Node;

use super::build::block_delimiter_value;
use super::builder::Builder;
use super::error::GrammarError;
use super::syntax::is_skipped;

impl<'a, 't> Builder<'a, 't> {
    pub(super) fn build_block(
        &mut self,
        name: &str,
        node: Node<'a, 't>,
    ) -> Result<Pattern, GrammarError> {
        let mut delimiters = node
            .children()
            .filter(|child| child.name() == "blockDelimiter");
        let (Some(open), Some(close)) = (delimiters.next(), delimiters.next()) else {
            unreachable!("the cpat grammar gives every block two delimiters");
        };
        let open = Literal::new("open", &block_delimiter_value(open))?;
        let close = Literal::new("close", &block_delimiter_value(close))?;

        let content = node
            .children()
            .find(|child| child.name() != "blockDelimiter" && !is_skipped(child))
            .expect("the cpat grammar gives every block content");

        // `...`: anything between balanced delimiters.
        if content.name() == "wildcard" {
            return Ok(Block::new(name, open, None, close).into());
        }
        let content = self.build_part(content)?;
        Ok(Block::new(name, open, Some(content), close).into())
    }
}
