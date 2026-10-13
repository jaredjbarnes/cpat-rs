//! One pattern per syntax node (v12 `Grammar._buildPattern`).

use crate::pattern::Pattern;
use crate::patterns::{
    Choice, Cut, Literal, Not, Optional, Regex, RightAssociated, Sequence, TakeUntil,
};
use crate::tree::Node;

use super::anonymous::is_anonymous;
use super::builder::{Builder, ChoiceSite, group_inner};
use super::depth::MAX_BUILD_DEPTH;
use super::error::GrammarError;
use super::escapes::decode_escapes;
use super::syntax::{Syntax, flattened, is_skipped, line_and_column};

impl<'a, 't> Builder<'a, 't> {
    /// The pattern `node` describes, called `name`. A part of a bigger
    /// pattern gets the next anonymous name; a literal or regex there is
    /// named by its content instead.
    pub(super) fn build_pattern(
        &mut self,
        name: &str,
        node: Node<'a, 't>,
    ) -> Result<Pattern, GrammarError> {
        if self.depth >= MAX_BUILD_DEPTH {
            let (line, column) = line_and_column(self.text, node.start());
            return Err(GrammarError::NestedTooDeep { line, column });
        }
        self.depth += 1;
        let pattern = self.build_syntax(name, node);
        self.depth -= 1;
        pattern
    }

    fn build_syntax(&mut self, name: &str, node: Node<'a, 't>) -> Result<Pattern, GrammarError> {
        match Syntax::of(&node) {
            Syntax::Literal => {
                let token = string_value(node.value());
                let name = if is_anonymous(name) { &token } else { name };
                Ok(Literal::new(name, &token)?.into())
            }
            Syntax::Regex => {
                let source = regex_source(node.value());
                let name = if is_anonymous(name) { source } else { name };
                Ok(Regex::new(name, source)?.into())
            }
            Syntax::Sequence => self.build_sequence(name, node),
            Syntax::Options => self.build_choice(name, node),
            Syntax::Repeat => self.build_repeat(name, node),
            Syntax::Block => self.build_block(name, node),
            Syntax::Group => {
                let inner = group_inner(node).ok_or(GrammarError::EmptyPatternGroup)?;
                self.build_part(inner)
            }
            Syntax::Not => {
                let inner = self.build_part(operand(node, "not"))?;
                Ok(Not::new(&format!("not-{}", inner.name()), inner).into())
            }
            Syntax::Optional => {
                let inner = self.build_part(operand(node, "?"))?;
                Ok(Optional::new(&format!("optional-{}", inner.name()), inner).into())
            }
            Syntax::RightAssociation => {
                let inner = self.build_part(operand(node, "right"))?;
                Ok(RightAssociated::new(inner).into())
            }
            // The rule's own name, not `name`: a choice recognises its
            // self-references by it. An alias renames afterwards.
            Syntax::Identifier => Ok(self.pattern_named(node.value())),
            Syntax::TakeUntil => {
                let terminator = node
                    .children()
                    .rev()
                    .find(|child| {
                        !is_skipped(child) && !matches!(child.name(), "anyChar" | "upTo" | "wall")
                    })
                    .expect("the cpat grammar gives `?->|` a terminator");
                let terminator = self.build_part(terminator)?;
                Ok(TakeUntil::new(name, terminator).into())
            }
            Syntax::BlockDelimiter => Err(GrammarError::LoneBlockDelimiter {
                delimiter: block_delimiter_value(node),
            }),
            Syntax::Other => Err(GrammarError::UnbuildableNode {
                name: node.name().to_string(),
            }),
        }
    }

    /// A part of a bigger pattern, under the next anonymous name.
    pub(super) fn build_part(&mut self, node: Node<'a, 't>) -> Result<Pattern, GrammarError> {
        let name = self.setup.anonymous.next();
        self.build_pattern(&name, node)
    }

    /// `a + b ~/ c`: `~/` sequences like `+` and inserts a cut, which
    /// commits the nearest enclosing choice once the parse passes it.
    fn build_sequence(&mut self, name: &str, node: Node<'a, 't>) -> Result<Pattern, GrammarError> {
        let mut children = Vec::new();
        for child in flattened(node) {
            if child.name() == "~/" {
                children.push(Cut::default().into());
            } else if !is_skipped(&child) {
                children.push(self.build_part(child)?);
            }
        }
        Ok(Sequence::new(name, children)?.into())
    }

    /// `a | b`. Every `|` is one `Choice`; it classifies itself (ordered,
    /// literal set or Pratt) once its references can be resolved, at its
    /// first parse, so the pattern built never depends on the order rules
    /// are defined in.
    fn build_choice(&mut self, name: &str, node: Node<'a, 't>) -> Result<Pattern, GrammarError> {
        let alternatives: Vec<Node<'a, 't>> = flattened(node)
            .into_iter()
            .filter(|child| !is_skipped(child))
            .collect();
        let patterns = alternatives
            .iter()
            .map(|&alternative| self.build_part(alternative))
            .collect::<Result<Vec<_>, _>>()?;

        self.choices.push(ChoiceSite {
            rule: self.current_rule.clone(),
            alternatives,
        });
        Ok(Choice::new(name, patterns)?.into())
    }
}

/// The operand of a prefix or postfix operator node: its first child that
/// is neither punctuation nor the operator itself.
fn operand<'a, 't>(node: Node<'a, 't>, operator: &str) -> Node<'a, 't> {
    node.children()
        .find(|child| !is_skipped(child) && child.name() != operator)
        .expect("the cpat grammar gives every operator an operand")
}

/// A cpat string literal's text: without its quotes, escapes decoded.
pub(super) fn string_value(quoted: &str) -> String {
    decode_escapes(&quoted[1..quoted.len() - 1])
}

/// A cpat regex's source: without its slashes.
fn regex_source(slashed: &str) -> &str {
    &slashed[1..slashed.len() - 1]
}

/// The literal text of a block delimiter node (`["{"]` is `{`).
pub(super) fn block_delimiter_value(node: Node<'_, '_>) -> String {
    let literal = node
        .children()
        .find(|child| child.name() == "literal")
        .expect("the cpat grammar gives every block delimiter a literal");
    string_value(literal.value())
}

#[cfg(test)]
mod tests {
    //! Nodes the cpat grammar never produces where the builder meets them,
    //! made by hand-built patterns: v12 keeps an error for each, and so do
    //! these.

    use crate::grammar::GrammarOptions;
    use crate::grammar::builder::Builder;
    use crate::grammar::error::GrammarError;
    use crate::grammar::setup::Setup;
    use crate::pattern::Pattern;
    use crate::patterns::{Literal, Regex, Sequence};

    fn lit(name: &str, token: &str) -> Pattern {
        Literal::new(name, token).unwrap().into()
    }

    /// What the builder makes of the node `pattern` parses `text` into.
    fn build(mut pattern: Pattern, text: &str) -> Result<Pattern, GrammarError> {
        let result = pattern.exec(text);
        let root = result.tree().expect("the pattern matches").root();
        Builder::empty(text, Setup::top_level(GrammarOptions::default())).build_pattern("x", root)
    }

    #[test]
    fn an_empty_group_is_an_error() {
        let group = Sequence::new("patternGroupExpr", vec![lit("(", "("), lit(")", ")")]);
        assert_eq!(
            build(group.unwrap().into(), "()").unwrap_err(),
            GrammarError::EmptyPatternGroup
        );
        assert_eq!(
            GrammarError::EmptyPatternGroup.to_string(),
            "Empty pattern group."
        );
    }

    #[test]
    fn a_node_the_builder_has_no_rule_for_is_an_error() {
        let error = build(lit("wildcard", "..."), "...").unwrap_err();
        assert_eq!(
            error,
            GrammarError::UnbuildableNode {
                name: "wildcard".into()
            }
        );
        assert_eq!(error.to_string(), "Couldn't build node: wildcard.");
    }

    #[test]
    fn a_lone_block_delimiter_is_an_error() {
        let delimiter = Sequence::new(
            "blockDelimiter",
            vec![
                lit("[", "["),
                Regex::new("literal", r#""(?:\\.|[^"\\])*""#)
                    .unwrap()
                    .into(),
                lit("]", "]"),
            ],
        );
        let error = build(delimiter.unwrap().into(), r#"["{"]"#).unwrap_err();
        assert_eq!(
            error,
            GrammarError::LoneBlockDelimiter {
                delimiter: "{".into()
            }
        );
        assert_eq!(
            error.to_string(),
            r#"Block delimiter [{] must be used at both the start and end of a sequence. Example: ["{"] content ["}"]"#
        );
    }
}
