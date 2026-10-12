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
use crate::pattern_id::Meta;
use crate::patterns::Sequence;

use super::ChoiceMode;
use super::literal_set::{Entry, LiteralSet};
use super::pratt::{Operator, Pratt};
use super::precedence::{Association, Table};

/// A classified choice.
#[derive(Clone, Debug)]
pub(super) enum Ready {
    /// No operators: the atoms are the indices of the alternatives to try.
    Ordered(Vec<usize>),
    /// No operators, and every atom is a literal (directly or through one
    /// resolved reference).
    LiteralSet(Box<LiteralSet>),
    Pratt(Box<Pratt>),
}

impl Ready {
    pub(super) fn mode(&self) -> ChoiceMode {
        match self {
            Ready::Ordered(_) => ChoiceMode::Ordered,
            Ready::LiteralSet(_) => ChoiceMode::LiteralSet,
            Ready::Pratt(_) => ChoiceMode::Pratt,
        }
    }

    pub(super) fn atoms(&self) -> &[usize] {
        match self {
            Ready::Ordered(atoms) => atoms,
            Ready::LiteralSet(set) => &set.atoms,
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

/// Whether a choice may take the literal-set mode. Chosen by the
/// constructor (`Choice::new` or `Choice::ordered`) and read only when the
/// choice classifies.
#[derive(Clone, Copy, Debug)]
pub(super) enum LiteralSets {
    Allowed,
    Refused,
}

/// Classifies `alternatives` of the choice `choice`, whose self-references
/// are children named `self_name`.
pub(super) fn classify(
    alternatives: &[Pattern],
    choice: Name,
    self_name: Name,
    literal_sets: LiteralSets,
    resolve: &dyn Fn(Name) -> Option<Rc<Pattern>>,
) -> Result<Ready, PatternError> {
    // What each atom matches when it is a literal; `None` once one isn't.
    let mut literals = match literal_sets {
        LiteralSets::Allowed => Some(Vec::new()),
        LiteralSets::Refused => None,
    };
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
            literals = match (
                literals.take(),
                literal_entry(alternative, resolved.as_deref()),
            ) {
                (Some(mut entries), Some(entry)) => {
                    entries.push(entry);
                    Some(entries)
                }
                _ => None,
            };
            atoms.push(index);
            continue;
        };
        let is_self = |child: Option<&Pattern>| child.is_some_and(|c| c.meta().name == self_name);
        let starts = is_self(children.first());
        let ends = is_self(children.last());
        if !starts && !ends {
            literals = None;
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
        return Ok(match literals {
            Some(entries) => Ready::LiteralSet(Box::new(LiteralSet::new(atoms, entries))),
            None => Ready::Ordered(atoms),
        });
    }
    Ok(Ready::Pratt(Box::new(Pratt::new(
        atoms, prefixes, postfixes, infixes, table,
    ))))
}

/// What an atom matches when it is a literal: the alternative itself, or
/// a reference whose target resolved to one (v12 `_prepareLiteralSet`
/// sees through one reference; a marked literal is not a literal there).
/// The failure is recorded under the pattern that would fail: for a
/// reference, its instance, the target renamed after the reference.
fn literal_entry(alternative: &Pattern, resolved: Option<&Pattern>) -> Option<Entry> {
    match (alternative, resolved) {
        (Pattern::Literal(literal), _) => Some(Entry {
            token: literal.token().into(),
            meta: literal.meta,
        }),
        (Pattern::Reference(reference), Some(Pattern::Literal(target))) => Some(Entry {
            token: target.token().into(),
            meta: Meta {
                id: target.meta.id,
                name: reference.meta.name,
            },
        }),
        _ => None,
    }
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
