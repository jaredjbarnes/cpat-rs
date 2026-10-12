//! Sorts a choice's alternatives into Pratt roles (v12
//! `classifyExpressionPatterns`). An alternative is classified by where it
//! refers to the choice itself:
//!
//! | role    | self-reference      | example            |
//! |---------|---------------------|--------------------|
//! | atom    | none                | `int`, `"(" e ")"` |
//! | prefix  | last only           | `"-" e`            |
//! | postfix | first only          | `e "!"`            |
//! | infix   | first and last      | `e "+" e`          |
//!
//! Operators are kept without their self-references, so the parse loop
//! matches only the operator part and the precedence tree assembles the
//! operands.

use std::rc::Rc;

use crate::name::Name;
use crate::pattern::Pattern;
use crate::pattern_error::PatternError;
use crate::patterns::Sequence;

use super::ChoiceMode;
use super::pratt::{Operator, Pratt};
use super::precedence::{Association, Table};

/// A classified choice.
#[derive(Clone, Debug)]
pub(super) enum Ready {
    /// No operators: the atoms are the indices of the alternatives to try.
    Ordered(Vec<usize>),
    Pratt(Box<Pratt>),
}

impl Ready {
    pub(super) fn mode(&self) -> ChoiceMode {
        match self {
            Ready::Ordered(_) => ChoiceMode::Ordered,
            Ready::Pratt(_) => ChoiceMode::Pratt,
        }
    }

    pub(super) fn atoms(&self) -> &[usize] {
        match self {
            Ready::Ordered(atoms) => atoms,
            Ready::Pratt(pratt) => &pratt.atoms,
        }
    }
}

/// Whether classifying needs scopes: some alternative (seen through
/// `RightAssociated`) is a reference, and only its target can say whether
/// it is an operator.
pub(super) fn needs_scope(alternatives: &[Pattern]) -> bool {
    alternatives
        .iter()
        .any(|alternative| matches!(unmarked(alternative), Pattern::Reference(_)))
}

/// Classifies `alternatives` of the choice `choice`, whose self-references
/// are children named `self_name`.
pub(super) fn classify(
    alternatives: &[Pattern],
    choice: Name,
    self_name: Name,
    resolve: &dyn Fn(Name) -> Option<Rc<Pattern>>,
) -> Result<Ready, PatternError> {
    let mut atoms = Vec::new();
    let mut prefixes = Vec::new();
    let mut postfixes = Vec::new();
    let mut infixes = Vec::new();
    let mut table = Table::default();

    for (index, alternative) in alternatives.iter().enumerate() {
        let inner = unmarked(alternative);
        let resolved = match inner {
            Pattern::Reference(reference) => resolve(reference.target()),
            _ => None,
        };
        // Only a sequence can be an operator: `e "+" e` reads its children
        // in order. Any other pattern merely CONTAINS the choice and is an
        // operand; reading its first and last children as operands would
        // misread `b = a | "d"` as the postfix `a "d"`.
        let Some(children) = sequence_children(resolved.as_deref().unwrap_or(inner)) else {
            atoms.push(index);
            continue;
        };
        let is_self = |child: Option<&Pattern>| child.is_some_and(|c| c.meta().name == self_name);
        let starts = is_self(children.first());
        let ends = is_self(children.last());
        if !starts && !ends {
            atoms.push(index);
            continue;
        }

        let count = children.iter().filter(|c| is_self(Some(c))).count();
        // The operator's name, which keys its precedence and association and
        // names its nodes. Under a marker (which has no name) it is the
        // child's; a resolved reference's target answers to the reference's
        // name, as v12's resolved clone does.
        let name = inner.meta().name;
        let operator = |suffix: &str, part: &[Pattern]| -> Result<Operator, PatternError> {
            if part.is_empty() {
                return Err(PatternError::SelfOnlyAlternative {
                    choice: choice.as_str().to_string(),
                    alternative: name.as_str().to_string(),
                });
            }
            let sequence = Sequence::new(&format!("{}-{suffix}", name.as_str()), part.to_vec())?;
            Ok(Operator { sequence, name })
        };

        let len = children.len();
        if ends && count == 1 {
            prefixes.push(operator("prefix", &children[..len - 1])?);
            table.set_precedence(name, index);
        } else if starts && count == 1 {
            postfixes.push(operator("postfix", &children[1..])?);
            table.set_precedence(name, index);
        } else if starts && ends && len > 2 {
            infixes.push(operator("delimiter", &children[1..len - 1])?);
            table.set_precedence(name, index);
            let association = match alternative {
                Pattern::RightAssociated(_) => Association::Right,
                _ => Association::Left,
            };
            table.set_association(name, association);
        }
        // Anything else (`e e`, `"x" e e`) is neither an operand nor an
        // operator, and v12 drops it: it is never tried.
    }

    if prefixes.is_empty() && postfixes.is_empty() && infixes.is_empty() {
        return Ok(Ready::Ordered(atoms));
    }
    Ok(Ready::Pratt(Box::new(Pratt::new(
        atoms, prefixes, postfixes, infixes, table,
    ))))
}

/// The alternative without its `RightAssociated` marker.
fn unmarked(alternative: &Pattern) -> &Pattern {
    match alternative {
        Pattern::RightAssociated(marker) => marker.child(),
        _ => alternative,
    }
}

fn sequence_children(pattern: &Pattern) -> Option<&[Pattern]> {
    match pattern {
        Pattern::Sequence(sequence) => Some(sequence.children()),
        _ => None,
    }
}
