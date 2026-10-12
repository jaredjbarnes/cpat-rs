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
use super::pratt::{Operator, Pratt, Role};
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

    /// The classified child at `index` (`Pattern::at_path`). Without
    /// operators the children are the atoms.
    pub(super) fn child<'a>(
        &'a self,
        alternatives: &'a [Pattern],
        index: usize,
    ) -> Option<&'a Pattern> {
        match self {
            Ready::Ordered(_) | Ready::LiteralSet(_) => {
                self.atoms().get(index).map(|&i| &alternatives[i])
            }
            Ready::Pratt(pratt) => pratt.child(alternatives, index),
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
    // What each classified alternative became, in declaration order: the
    // choice's children as v12 lists them (`Pattern::at_path`).
    let mut roles = Vec::new();
    let mut table = Table::default();

    for (index, alternative) in alternatives.iter().enumerate() {
        let inner = unmarked(alternative);
        let resolved = match inner {
            Pattern::Reference(reference) => resolve(reference.target()),
            _ => None,
        };
        let seen = resolved.as_deref().unwrap_or(inner);
        let Some(role) = role_of(seen, self_name) else {
            // Neither an operand nor an operator (`e e`, `"x" e e`): v12
            // drops it, and it is never tried.
            continue;
        };
        roles.push(role);
        // The operator's name, which keys its precedence and association and
        // names its nodes. Under a marker (which has no name) it is the
        // child's; a resolved reference's target answers to the reference's
        // name, as v12's resolved clone does.
        let name = inner.meta().name;
        let operators = match role {
            Role::Atom => {
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
            }
            Role::Prefix => &mut prefixes,
            Role::Postfix => &mut postfixes,
            Role::Infix => {
                let association = match alternative {
                    Pattern::RightAssociated(_) => Association::Right,
                    _ => Association::Left,
                };
                table.set_association(name, association);
                &mut infixes
            }
        };

        let part = operator_part(role, sequence_children(seen).unwrap_or(&[]));
        if part.is_empty() {
            return Err(PatternError::SelfOnlyAlternative {
                choice: choice.as_str().to_string(),
                alternative: name.as_str().to_string(),
            });
        }
        let sequence = Sequence::new(&operator_name(role, name), part.to_vec())?;
        operators.push(Operator::new(sequence, name));
        table.set_precedence(name, index);
    }

    if prefixes.is_empty() && postfixes.is_empty() && infixes.is_empty() {
        return Ok(match literals {
            Some(entries) => Ready::LiteralSet(Box::new(LiteralSet::new(atoms, entries))),
            None => Ready::Ordered(atoms),
        });
    }
    Ok(Ready::Pratt(Box::new(Pratt::new(
        atoms, prefixes, postfixes, infixes, table, &roles,
    ))))
}

/// What an alternative becomes, seen through its marker and a reference
/// resolved one level (`seen`), in a choice whose self-references are
/// children named `self_name`. `None`: neither an operand nor an operator,
/// so it is dropped.
///
/// Only a sequence can be an operator: `e "+" e` reads its children in
/// order. Any other pattern merely CONTAINS the choice and is an operand;
/// reading its first and last children as operands would misread
/// `b = a | "d"` as the postfix `a "d"`.
pub(crate) fn role_of(seen: &Pattern, self_name: Name) -> Option<Role> {
    let Some(children) = sequence_children(seen) else {
        return Some(Role::Atom);
    };
    let is_self = |child: Option<&Pattern>| child.is_some_and(|c| c.meta().name == self_name);
    let starts = is_self(children.first());
    let ends = is_self(children.last());
    if !starts && !ends {
        return Some(Role::Atom);
    }

    let count = children.iter().filter(|c| is_self(Some(c))).count();
    if ends && count == 1 {
        Some(Role::Prefix)
    } else if starts && count == 1 {
        Some(Role::Postfix)
    } else if starts && ends && children.len() > 2 {
        Some(Role::Infix)
    } else {
        None
    }
}

/// An operator's part of its sequence's `children`: the children without
/// the self-references (all of them for an atom).
pub(crate) fn operator_part(role: Role, children: &[Pattern]) -> &[Pattern] {
    let len = children.len();
    match role {
        Role::Atom => children,
        Role::Prefix => &children[..len.saturating_sub(1)],
        Role::Postfix => &children[len.min(1)..],
        Role::Infix if len >= 2 => &children[1..len - 1],
        Role::Infix => &[],
    }
}

/// The name of the operator made from the alternative called `name`
/// (v12's `<name>-prefix`, `-postfix`, `-delimiter`).
pub(crate) fn operator_name(role: Role, name: Name) -> String {
    let suffix = match role {
        Role::Atom => return name.as_str().to_string(),
        Role::Prefix => "prefix",
        Role::Postfix => "postfix",
        Role::Infix => "delimiter",
    };
    format!("{}-{suffix}", name.as_str())
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
pub(crate) fn unmarked(alternative: &Pattern) -> &Pattern {
    match alternative {
        Pattern::RightAssociated(marker) => marker.child(),
        _ => alternative,
    }
}

pub(crate) fn sequence_children(pattern: &Pattern) -> Option<&[Pattern]> {
    match pattern {
        Pattern::Sequence(sequence) => Some(sequence.children()),
        _ => None,
    }
}
