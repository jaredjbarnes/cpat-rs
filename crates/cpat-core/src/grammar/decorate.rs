//! Applying the decorators written above a rule (v12 `_applyDecorators`).

use serde_json::Value;

use crate::pattern::Pattern;
use crate::tree::Node;

use super::builder::Builder;
use super::error::GrammarError;

impl Builder<'_, '_> {
    /// `pattern` after the decorators above the statement at `index`, nearest
    /// first, as v12 applies them. The walk back passes comments and blank
    /// lines and stops at anything else.
    pub(super) fn decorate(
        &self,
        statements: &[Node<'_, '_>],
        index: usize,
        mut pattern: Pattern,
    ) -> Result<Pattern, GrammarError> {
        let decorations = statements[..index]
            .iter()
            .rev()
            .filter(|statement| !matches!(statement.name(), "comment" | "newLine"))
            .map_while(decoration);
        for statement in decorations {
            pattern = self.apply(statement, pattern)?;
        }
        Ok(pattern)
    }

    /// One `@name` or `@name(json)` statement applied to `pattern`.
    fn apply(&self, statement: Node<'_, '_>, pattern: Pattern) -> Result<Pattern, GrammarError> {
        let name = statement
            .children()
            .find(|child| child.name() == "decorationName")
            .expect("the cpat grammar names every decorator")
            .value();
        let Some(decorator) = self.setup.decorators.get(name) else {
            return Err(GrammarError::UnknownDecorator {
                decorator: name.to_string(),
                pattern: pattern.name().to_string(),
                known: self.setup.decorators.names().map(String::from).collect(),
            });
        };
        let argument = argument(name, statement)?;

        let from = pattern.name();
        let decorated = decorator(pattern, argument.as_ref())
            .map_err(|message| GrammarError::Decorator { message })?;
        if decorated.name() != from {
            return Err(GrammarError::DecoratorRenamed {
                decorator: name.to_string(),
                from: from.to_string(),
                to: decorated.name().to_string(),
            });
        }
        Ok(decorated)
    }
}

/// The decorator statement `statement` is, else `None` (the walk stops). The
/// cpat grammar's decoration choice gives its alternative's node, so
/// `decorationStatement` itself is only unwrapped in case it ever doesn't.
fn decoration<'a, 't>(statement: &Node<'a, 't>) -> Option<Node<'a, 't>> {
    match statement.name() {
        "methodDecorationStatement" | "nameDecorationStatement" => Some(*statement),
        "decorationStatement" => statement.children().next(),
        _ => None,
    }
}

/// The statement's JSON argument: none for `@name` and `@name()`.
fn argument(name: &str, statement: Node<'_, '_>) -> Result<Option<Value>, GrammarError> {
    let Some(json) = statement.children().find(|child| {
        matches!(
            child.name(),
            "jsonArray" | "jsonObject" | "jsonString" | "jsonNumber" | "jsonBoolean" | "jsonNull"
        )
    }) else {
        return Ok(None);
    };
    serde_json::from_str(json.value())
        .map(Some)
        .map_err(|error| GrammarError::DecoratorArgument {
            decorator: name.to_string(),
            message: error.to_string(),
        })
}
