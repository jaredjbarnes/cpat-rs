//! A pattern instance at a place in the grammar, with the chain of places
//! above it: v12's `parent` chain, rebuilt for the length of one query.
//!
//! Patterns keep no parent pointers (DESIGN §2), but v12's introspection
//! is defined by them: "what comes after me" asks the parent, and "am I
//! already being expanded" counts ancestors with my id. A `Loc` is a link
//! in such a chain, made top-down from the root along a `PatternPath` and
//! further down as a query expands patterns (through references, too). The
//! chain lives only while the query runs; nothing is written into patterns.

use std::rc::Rc;

use crate::name::Name;
use crate::pattern::Pattern;
use crate::pattern_id::{Meta, PatternId};
use crate::pattern_path::{PatternPath, step};
use crate::patterns::{Choice, Role, operator_name, operator_part, role_of, sequence_children};
use crate::scope::Scope;

use super::Introspection;
use super::frames::{self, FrameRef, Frames, Target};

/// How far a query may expand below the place it was asked about. A grammar whose expansion never ends
/// (`a = "x"? a`: the optional lets the sequence reach its recursive second
/// child without consuming anything, and only a FIRST child is checked for
/// recursion) makes v12 recurse until the JS engine throws `RangeError`.
/// Here the expansion stops at this depth and the query answers with what
/// it found, so no grammar can overflow the stack. Real grammars nest a few
/// dozen levels; this is far above that and far below the stack (a level
/// costs a few calls of a few hundred bytes each in a debug build).
///
/// It counts from the asked-about place, not from the root: the path there
/// came from a parse, which nests as deep as its own limit allows (a leaf
/// under 90 parentheses is 271 steps down), and walking it is bounded by
/// its length, so the walk itself never stops early.
pub(super) const MAX_DEPTH: usize = 256;

/// How many places one query may make. The depth limit bounds the stack,
/// not the work: an expansion that never ends and branches on the way
/// (`a = !"z" a`, where the lookahead offers what follows it, which is `a`
/// again) makes twice as many places per level, so 256 levels would never
/// finish. Past this many places a query stops expanding and answers with
/// what it found. Counted, not timed, so the answer is the same on every
/// machine. Real queries are tiny next to it: every query over the grammars
/// of v12's tests (the expression grammar, the value grammar, blocks,
/// contexts) made at most 31 places, measured in milestone 8.
pub(super) const BUDGET: usize = 100_000;

pub(super) struct Loc<'r> {
    pub(super) node: Node<'r>,
    /// The id the recursion checks compare. An operator has none of its
    /// own: v12 mints a fresh id for it, which no other pattern shares.
    pub(super) id: Option<PatternId>,
    /// The name this instance answers to: a reference's target and a
    /// context's child are renamed after the reference, as `clone(name)`
    /// renames v12's resolved instance.
    pub(super) name: Name,
    /// The reference name this instance was renamed to, which a context
    /// passes on to its child (`Pattern::rename`).
    renamed: Option<Name>,
    /// This instance's index in its parent's children.
    pub(super) step: u32,
    pub(super) parent: Option<Rc<Loc<'r>>>,
    /// The scopes a parse would have here (references resolve against them).
    frames: Frames<'r>,
    depth: usize,
}

#[derive(Clone, Copy)]
pub(super) enum Node<'r> {
    /// A pattern in the tree, never a `RightAssociated` marker (a marker is
    /// no step, as in `Pattern::at_path`).
    Pattern(&'r Pattern),
    /// One of a choice's operators: the alternative's sequence without its
    /// self-references. `pattern` is the operator itself when the choice
    /// is classified, otherwise the alternative it is made from.
    Operator {
        pattern: &'r Pattern,
        part: &'r [Pattern],
    },
    /// A context reached through a reference to its own name.
    OwnContext {
        scope: &'r Scope,
        owner: Meta,
        child: &'r Pattern,
    },
}

/// A choice's child, as v12 classifies it.
pub(super) struct Classified<'r> {
    pub(super) role: Role,
    node: Node<'r>,
    id: Option<PatternId>,
    name: Name,
    /// An operator made from a reference's target parses that target's
    /// parts under the target's frames.
    frames: Frames<'r>,
}

impl<'r> Loc<'r> {
    pub(super) fn root(root: &'r Pattern, bottom: FrameRef<'r>) -> Rc<Loc<'r>> {
        let pattern = unmarked(root);
        Rc::new(Loc {
            node: Node::Pattern(pattern),
            id: Some(pattern.id()),
            name: pattern.meta().name,
            renamed: None,
            step: 0,
            parent: None,
            frames: Rc::from([bottom]),
            depth: 0,
        })
    }

    /// The pattern this place holds: for an operator, the operator (or
    /// what it is made from); for a context found by its own name, its
    /// child, since that context is built at lookup and is in no tree.
    pub(super) fn pattern(&self) -> &'r Pattern {
        match self.node {
            Node::Pattern(pattern) | Node::Operator { pattern, .. } => pattern,
            Node::OwnContext { child, .. } => child,
        }
    }

    /// The child indices from the root to here.
    pub(super) fn path(&self) -> PatternPath {
        let mut steps = Vec::with_capacity(self.depth);
        let mut at = self;
        while let Some(parent) = &at.parent {
            steps.push(at.step);
            at = parent;
        }
        steps.reverse();
        PatternPath::new(&steps)
    }

    /// v12 `isRecursivePattern`: this pattern is already being expanded
    /// twice further up the chain.
    pub(super) fn is_recursive(&self) -> bool {
        let Some(id) = self.id else {
            return false;
        };
        let mut count = 0;
        let mut at = self.parent.as_deref();
        while let Some(loc) = at {
            if loc.id == Some(id) {
                count += 1;
                if count > 1 {
                    return true;
                }
            }
            at = loc.parent.as_deref();
        }
        false
    }
}

impl<'r> Introspection<'r> {
    /// The place `path` leads to from the root, or nothing for a path
    /// longer than `max_path`. The walk follows a given path, so neither
    /// the depth limit nor the budget applies to it; the expansion below
    /// the place it reaches gets `MAX_DEPTH` levels.
    ///
    /// The length check is what bounds the stack here (fix round 2): a
    /// path is a public argument, and every step adds a link to the chain,
    /// which a climb recurses up and a drop recurses down. A 300 000-step
    /// path through `expr = "(" expr ")" | "x"` aborted the process. No
    /// parse under the same limits makes a path longer than `max_path`
    /// (every step below the root enters a composite, which takes a frame,
    /// or ends at a leaf), so a real path is never refused.
    pub(super) fn walk(&self, path: &PatternPath) -> Option<Rc<Loc<'r>>> {
        if path.steps().len() > self.max_path {
            return None;
        }
        let loc = path
            .steps()
            .iter()
            .try_fold(self.root_loc(), |loc, &index| {
                self.step_to(&loc, usize::try_from(index).ok()?)
            })?;
        self.floor.set(loc.depth);
        Some(loc)
    }

    /// Whether `loc` is as deep as this query may expand (`MAX_DEPTH`
    /// below the place it was asked about).
    pub(super) fn is_too_deep(&self, loc: &Loc<'r>) -> bool {
        loc.depth >= self.floor.get() + MAX_DEPTH
    }

    /// The child at `index`, numbered as `Pattern::at_path` numbers it, as
    /// an expansion makes it: counted against both bounds.
    pub(super) fn child(&self, loc: &Rc<Loc<'r>>, index: usize) -> Option<Rc<Loc<'r>>> {
        if self.is_too_deep(loc) || !self.spend() {
            return None;
        }
        self.step_to(loc, index)
    }

    /// The child at `index`, unbounded: one step of a walk or an expansion.
    fn step_to(&self, loc: &Rc<Loc<'r>>, index: usize) -> Option<Rc<Loc<'r>>> {
        let pattern = match loc.node {
            Node::Operator { part, .. } => part.get(index)?,
            Node::OwnContext {
                scope,
                owner,
                child,
            } => {
                let owner = Meta {
                    id: owner.id,
                    name: loc.name,
                };
                let frame = FrameRef::Scope { scope, owner };
                return (index == 0).then(|| self.context_child(loc, child, frame));
            }
            Node::Pattern(pattern) => match pattern {
                Pattern::Literal(_) | Pattern::Regex(_) | Pattern::Cut(_) | Pattern::Custom(_) => {
                    return None;
                }
                Pattern::Choice(choice) => {
                    let mut classified = self.classified(loc, choice);
                    if index >= classified.len() {
                        return None;
                    }
                    return Some(self.choice_child(loc, classified.swap_remove(index), index));
                }
                Pattern::Reference(reference) => {
                    if index != 0 {
                        return None;
                    }
                    let (target, frames) = frames::resolve(reference, &loc.frames, &self.live)?;
                    return Some(self.target(loc, target, frames));
                }
                Pattern::Context(context) => {
                    let frame = FrameRef::of_context(context, loc.name);
                    return (index == 0).then(|| self.context_child(loc, context.child(), frame));
                }
                Pattern::Sequence(p) => p.children().get(index)?,
                Pattern::Repeat(p) => p.child_at(index)?,
                Pattern::Block(p) => p.child_at(index)?,
                Pattern::Optional(p) => (index == 0).then_some(p.child())?,
                Pattern::Not(p) => (index == 0).then_some(p.child())?,
                Pattern::TakeUntil(p) => (index == 0).then_some(p.terminator())?,
                // Never held (locs are unmarked); a marker's child is
                // its child.
                Pattern::RightAssociated(p) => (index == 0).then_some(p.child())?,
            },
        };
        Some(self.placed(loc, step(index), pattern, Rc::clone(&loc.frames)))
    }

    /// Counts one more place against this query's `BUDGET`; `false` once
    /// it is spent.
    pub(super) fn spend(&self) -> bool {
        let spent = self.spent.get();
        if spent >= BUDGET {
            return false;
        }
        self.spent.set(spent + 1);
        true
    }

    /// The roles of the classified children of the choice at `loc`, or
    /// `None` when `loc` holds something else.
    pub(super) fn roles(&self, loc: &Rc<Loc<'r>>) -> Option<Vec<Role>> {
        match loc.node {
            Node::Pattern(Pattern::Choice(choice)) => Some(
                self.classified(loc, choice)
                    .into_iter()
                    .map(|child| child.role)
                    .collect(),
            ),
            _ => None,
        }
    }

    /// A choice's children in v12's classified order: atoms are the
    /// alternatives themselves, operators their sequences without the
    /// self-references; alternatives that are neither are left out. A
    /// reference alternative is resolved one level to see whether it is
    /// an operator, as classification does (DESIGN §8).
    pub(super) fn classified(&self, loc: &Rc<Loc<'r>>, choice: &'r Choice) -> Vec<Classified<'r>> {
        let mut children = Vec::new();
        for alternative in choice.original_alternatives() {
            let inner = unmarked(alternative);
            let (seen, frames) = match inner {
                Pattern::Reference(reference) => {
                    match frames::resolve(reference, &loc.frames, &self.live) {
                        Some((Target::Pattern(target), frames)) => (unmarked(target), frames),
                        // A context, or a name that resolves to nothing:
                        // an operand, as classification decides.
                        _ => (inner, Rc::clone(&loc.frames)),
                    }
                }
                _ => (inner, Rc::clone(&loc.frames)),
            };
            let Some(role) = role_of(seen, choice.self_name()) else {
                continue;
            };
            let classified = match role {
                Role::Atom => Classified {
                    role,
                    node: Node::Pattern(inner),
                    id: Some(inner.id()),
                    name: inner.meta().name,
                    frames: Rc::clone(&loc.frames),
                },
                _ => Classified {
                    role,
                    node: Node::Operator {
                        // The operator itself once the choice is classified
                        // (what `at_path` hands out), else its source.
                        pattern: choice.child_at(children.len()).unwrap_or(seen),
                        part: operator_part(role, sequence_children(seen).unwrap_or(&[])),
                    },
                    id: None,
                    name: Name::new(&operator_name(role, inner.meta().name)),
                    frames,
                },
            };
            children.push(classified);
        }
        children
    }

    /// The place of a choice's classified child `index`.
    pub(super) fn choice_child(
        &self,
        loc: &Rc<Loc<'r>>,
        child: Classified<'r>,
        index: usize,
    ) -> Rc<Loc<'r>> {
        Rc::new(Loc {
            node: child.node,
            id: child.id,
            name: child.name,
            renamed: None,
            step: step(index),
            parent: Some(Rc::clone(loc)),
            frames: child.frames,
            depth: loc.depth + 1,
        })
    }

    /// `pattern` as child `step` of `loc`, under `frames`.
    fn placed(
        &self,
        loc: &Rc<Loc<'r>>,
        step: u32,
        pattern: &'r Pattern,
        frames: Frames<'r>,
    ) -> Rc<Loc<'r>> {
        let pattern = unmarked(pattern);
        Rc::new(Loc {
            node: Node::Pattern(pattern),
            id: Some(pattern.id()),
            name: pattern.meta().name,
            renamed: None,
            step,
            parent: Some(Rc::clone(loc)),
            frames,
            depth: loc.depth + 1,
        })
    }

    /// What a reference resolved to, as its child 0: renamed after the
    /// reference.
    fn target(&self, loc: &Rc<Loc<'r>>, target: Target<'r>, frames: Frames<'r>) -> Rc<Loc<'r>> {
        let (node, id) = match target {
            Target::Pattern(pattern) => {
                let pattern = unmarked(pattern);
                (Node::Pattern(pattern), pattern.id())
            }
            Target::OwnContext {
                scope,
                owner,
                child,
            } => (
                Node::OwnContext {
                    scope,
                    owner,
                    child,
                },
                owner.id,
            ),
        };
        Rc::new(Loc {
            node,
            id: Some(id),
            name: loc.name,
            renamed: Some(loc.name),
            step: 0,
            parent: Some(Rc::clone(loc)),
            frames,
            depth: loc.depth + 1,
        })
    }

    /// A context's child, under the context's frame. A context renamed by
    /// a reference renames its child too (`Pattern::rename`).
    fn context_child(
        &self,
        loc: &Rc<Loc<'r>>,
        child: &'r Pattern,
        frame: FrameRef<'r>,
    ) -> Rc<Loc<'r>> {
        let frames: Frames<'r> = loc.frames.iter().copied().chain([frame]).collect();
        let child = unmarked(child);
        Rc::new(Loc {
            node: Node::Pattern(child),
            id: Some(child.id()),
            name: loc.renamed.unwrap_or(child.meta().name),
            renamed: loc.renamed,
            step: 0,
            parent: Some(Rc::clone(loc)),
            frames,
            depth: loc.depth + 1,
        })
    }
}

/// The pattern without `RightAssociated` markers around it.
fn unmarked(pattern: &Pattern) -> &Pattern {
    match pattern {
        Pattern::RightAssociated(marker) => unmarked(marker.child()),
        _ => pattern,
    }
}
