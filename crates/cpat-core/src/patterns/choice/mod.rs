//! The one choice pattern (DESIGN §8): `a | b | c`, in one of two modes
//! picked by classifying its alternatives.

mod classify;
mod ordered;
mod pratt;
mod precedence;

use std::rc::Rc;

use crate::cursor::Cursor;
use crate::halt::Halt;
use crate::name::Name;
use crate::pattern::Pattern;
use crate::pattern_error::PatternError;
use crate::pattern_id::Meta;
use crate::patterns::Sequence;
use crate::tree::NodeId;

use classify::Ready;

/// `a | b | c`.
///
/// Its alternatives decide how it parses (v12 `Expression`):
///
/// - **Ordered**: no alternative refers back to the choice. The first
///   alternative that matches wins, and its node is returned as it is.
/// - **Pratt**: some alternatives are operators, sequences that refer back
///   to the choice by name: `"-" expr` (prefix), `expr "!"` (postfix),
///   `expr "+" expr` (infix). Operands and operators are parsed in a loop
///   and assembled by precedence, which is declaration order: earlier binds
///   tighter. Infix operators associate left unless wrapped in
///   `RightAssociated`. The result is a tree of `NodeKind::Expression` nodes
///   named after the operator alternatives.
///
/// Only a sequence can be an operator. An alternative that merely contains
/// the choice (another choice, a repeat) is an operand: `b = a | "d"`
/// inside `a` is not the postfix `a "d"`.
///
/// **Known limitation (v12's):** a prefix operator that consumes the last
/// token does not backtrack. With `x = "a" x | "a"`, the input `a` fails:
/// the prefix takes `a` and no operand is left.
#[derive(Clone, Debug)]
pub struct Choice {
    pub(crate) meta: Meta,
    // The name a self-reference has. v12's `_originalName`: a renamed clone
    // (a reference's instance) still recognises its own alternatives.
    self_name: Name,
    pub(crate) alternatives: Vec<Pattern>,
    // `None` until classified. That is only ever the case when an
    // alternative is a reference: whether its target is an operator
    // depends on what the name resolves to, which only the scopes at parse
    // time know (DESIGN §8). Clones keep the classification.
    ready: Option<Ready>,
}

/// How a choice parses, decided by classifying its alternatives.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChoiceMode {
    /// Every alternative is an operand: the first that matches wins.
    Ordered,
    /// Some alternatives are operators: operands and operators are
    /// assembled by precedence.
    Pratt,
}

impl Choice {
    /// A choice between `alternatives`, classified right away unless one of
    /// them is a reference (then at its first parse, when the reference
    /// can be resolved).
    ///
    /// Fails when there are no alternatives, or when an alternative is a
    /// sequence of nothing but a reference to the choice itself.
    pub fn new(name: &str, alternatives: Vec<Pattern>) -> Result<Choice, PatternError> {
        if alternatives.is_empty() {
            return Err(PatternError::NoChildren {
                name: name.to_string(),
                kind: "choice",
            });
        }

        let meta = Meta::new(name);
        let mut choice = Choice {
            meta,
            self_name: meta.name,
            alternatives: alternatives.into_iter().map(Pattern::adopted).collect(),
            ready: None,
        };
        if !classify::needs_scope(&choice.alternatives) {
            choice.classify_with(&|_| None)?;
        }
        Ok(choice)
    }

    /// The alternatives as they were given (v12 `originalPatterns`).
    pub fn original_alternatives(&self) -> &[Pattern] {
        &self.alternatives
    }

    /// `None` until the choice is classified: only a choice with a
    /// reference alternative waits, until its first parse.
    pub fn mode(&self) -> Option<ChoiceMode> {
        self.ready.as_ref().map(Ready::mode)
    }

    /// The operands, in order. Empty until classified.
    pub fn atom_patterns(&self) -> impl Iterator<Item = &Pattern> {
        let atoms = self.ready.as_ref().map_or(&[][..], Ready::atoms);
        atoms.iter().map(|&i| &self.alternatives[i])
    }

    /// The prefix operators without their trailing self-reference, each
    /// named `<alternative>-prefix`. Empty in ordered mode.
    pub fn prefix_patterns(&self) -> impl Iterator<Item = &Sequence> {
        self.operators(|pratt| &pratt.prefixes)
    }

    /// The postfix operators without their leading self-reference, each
    /// named `<alternative>-postfix`. Empty in ordered mode.
    pub fn postfix_patterns(&self) -> impl Iterator<Item = &Sequence> {
        self.operators(|pratt| &pratt.postfixes)
    }

    /// The infix operators without their two self-references, each named
    /// `<alternative>-delimiter`. Empty in ordered mode.
    pub fn infix_patterns(&self) -> impl Iterator<Item = &Sequence> {
        self.operators(|pratt| &pratt.infixes)
    }

    fn operators(
        &self,
        list: impl Fn(&pratt::Pratt) -> &[pratt::Operator],
    ) -> impl Iterator<Item = &Sequence> {
        let operators = match &self.ready {
            Some(Ready::Pratt(pratt)) => list(pratt),
            _ => &[],
        };
        operators.iter().map(|operator| &operator.sequence)
    }

    /// Classifies the alternatives, resolving a reference alternative's
    /// target with `resolve`. The parse does this on first use with the
    /// cursor's scopes; a builder that knows the definitions can do it
    /// earlier. An unresolved reference is an operand (parsing it later
    /// reports the unknown name).
    pub(crate) fn classify_with(
        &mut self,
        resolve: &dyn Fn(Name) -> Option<Rc<Pattern>>,
    ) -> Result<(), PatternError> {
        let ready =
            classify::classify(&self.alternatives, self.meta.name, self.self_name, resolve)?;
        self.ready = Some(ready);
        Ok(())
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();

        if self.ready.is_none() {
            let resolve = |name| cursor.lookup(name).map(|found| found.master);
            if let Err(error) = self.classify_with(&resolve) {
                cursor.halt(first, self.meta, Halt::Pattern(error));
                return None;
            }
        }
        let ready = self.ready.as_mut().expect("classified above");

        // Nothing to start an expression with: nothing can match.
        if ready.atoms().is_empty() {
            cursor.record_error(first, first, self.meta);
            return None;
        }

        // Either way the choice owns a cut scope: a cut inside one of its
        // alternatives commits this choice, not one further out.
        let alternatives = &mut self.alternatives;
        let (node, committed) = cursor.in_choice_scope(|cursor| match ready {
            Ready::Ordered(atoms) => ordered::run(alternatives, atoms, first, cursor),
            Ready::Pratt(pratt) => pratt.run(alternatives, cursor),
        });

        if cursor.halted() {
            return None;
        }

        if let Some(node) = node {
            cursor.resolve_error();
            return Some(node);
        }

        cursor.set_pos(first);

        // A committed failure keeps the failed alternative's deep error: that
        // branch's diagnosis is the one the grammar author meant, not a
        // shallow "no alternative matched here". A committed alternative
        // that left no error matched nothing (a cut and absent optionals),
        // so the choice is that zero-width success too, as in v12.
        if !committed {
            cursor.record_error(first, first, self.meta);
        }

        None
    }
}

/// v12's recursion guard for a choice's atoms: an alternative that is
/// already parsing at this very spot higher up (an outer recursion level of
/// it) would only repeat that attempt, so it is skipped. This is what ends
/// mutual recursion like `a = b | "c"`, `b = a | "d"` before the
/// references' own guard has to.
fn already_parsing_here(cursor: &Cursor<'_>, atom: &Pattern, pos: usize) -> bool {
    cursor.ancestors_at(atom.id(), pos, 1) > 0
}
