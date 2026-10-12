//! Readies a root pattern's references before `exec` parses it: builds the
//! implicit scope from the root's tree and checks that every reference the
//! parse could reach names something (DESIGN §3).

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::name::Name;
use crate::pattern::Pattern;
use crate::pattern_error::PatternError;
use crate::patterns::{Reference, RootLink};
use crate::scope::{self, Scope, ScopeFrame, TargetKey};

/// The implicit scope for `root`, or `None` when its tree has no reference
/// (then nothing can ever be looked up). Built and checked once: the result
/// is cached on the root tree's first reference and reused while the root
/// is the same pattern.
pub(crate) fn prepare(root: &mut Pattern) -> Result<Option<Rc<Scope>>, PatternError> {
    let Some(first) = first_reference(root) else {
        return Ok(None);
    };
    if let Some(scope) = readied_scope(root, first) {
        return Ok(Some(scope));
    }
    let key = (root.id(), root.meta().name);

    let scope = Rc::new(Scope::implicit(root));
    check_with(root, &scope, CHECK_BUDGET)?;

    if let Some(first) = first_reference_mut(root) {
        first.set_root_link(RootLink {
            root: key,
            scope: Rc::clone(&scope),
        });
    }
    Ok(Some(scope))
}

/// Whether every reference reachable from `root` resolves. See
/// `Pattern::check`.
///
/// Uses the scope `exec` readied for `root` when there is one: references
/// that already resolved there keep their targets' homes in that scope
/// (`scope::home_applies`), so a new one would send them elsewhere.
pub(crate) fn check(root: &Pattern) -> Result<(), PatternError> {
    let scope = first_reference(root)
        .and_then(|first| readied_scope(root, first))
        .unwrap_or_else(|| Rc::new(Scope::implicit(root)));
    check_with(root, &scope, CHECK_BUDGET)
}

/// The implicit scope `exec` readied for `root`, borrowed from its first
/// reference, if `root` was exec'd as it is now. Introspection looks names
/// up in it so that references a parse already resolved keep their homes
/// (`scope::home_applies` compares scopes by identity).
pub(crate) fn readied_scope_ref(root: &Pattern) -> Option<&Scope> {
    let key = (root.id(), root.meta().name);
    first_reference(root)?
        .root_link()
        .filter(|link| link.root == key)
        .map(|link| &*link.scope)
}

/// The scope cached on `root`'s first reference, if it was readied for
/// this very root.
fn readied_scope(root: &Pattern, first: &Reference) -> Option<Rc<Scope>> {
    let key = (root.id(), root.meta().name);
    first
        .root_link()
        .filter(|link| link.root == key)
        .map(|link| Rc::clone(&link.scope))
}

fn check_with(root: &Pattern, scope: &Rc<Scope>, budget: usize) -> Result<(), PatternError> {
    let frames = vec![ScopeFrame::new(Rc::clone(scope), root.meta())];
    let mut checker = Checker::default();
    checker.walk_state(root, frames)?;

    while let Some(state) = checker.pending.pop() {
        if checker.states > budget {
            // Gave up: the rest is checked lazily, when a parse reaches it.
            return Ok(());
        }
        checker.walk_state(&state.pattern, state.frames)?;
    }
    Ok(())
}

/// How many (target, scope resolution) states the up-front check walks
/// before it stops and leaves the rest to the parse. Grammars with many
/// mutually referring nested contexts can have exponentially many distinct
/// resolutions; v12 never meets them because it looks names up lazily. Past
/// this budget, an unknown name is still reported, as a halt when a parse
/// reaches it (`Halt::Pattern`), which is v12's behavior.
const CHECK_BUDGET: usize = 10_000;

/// Walks every pattern a parse could reach, pushing a context's scope while
/// inside it and following each reference into what it would resolve to,
/// with the scope stack it would have then.
///
/// Iterative between references: a reference adds a state to `pending`
/// instead of recursing, so the native stack only goes as deep as one
/// pattern's own tree, never as deep as the chain of references. A target
/// already walked under the same resolution (`scope::resolution`) is
/// skipped, which ends recursive grammars.
#[derive(Default)]
struct Checker {
    seen: HashSet<(TargetKey, u32)>,
    pending: Vec<State>,
    states: usize,
    // Each distinct resolution gets a small id, so a reference costs one
    // hash of a pair, not of a whole table.
    resolutions: HashMap<Vec<(Name, TargetKey)>, u32>,
    // The resolution id of each frame stack met so far, by the frames'
    // identities: stacks repeat far more often than resolutions change, and
    // a resolution costs a pass over every name in scope.
    stacks: HashMap<Vec<(*const Scope, Name)>, u32>,
    // The id of the current frames' resolution, one entry per frame stack
    // depth seen in this state (it changes only on entering a context).
    current: Vec<u32>,
}

/// A reference's target to walk, under the scopes it would parse with.
struct State {
    pattern: Pattern,
    frames: Vec<ScopeFrame>,
}

impl Checker {
    fn walk_state(
        &mut self,
        pattern: &Pattern,
        mut frames: Vec<ScopeFrame>,
    ) -> Result<(), PatternError> {
        self.states += 1;
        self.current.clear();
        self.enter(&frames);
        // Children are queued last first so the first reference in parse
        // order is walked first, keeping the reported name stable.
        let first_queued = self.pending.len();
        self.walk(pattern, &mut frames)?;
        self.pending[first_queued..].reverse();
        Ok(())
    }

    /// Notes the resolution of `frames`, now the innermost stack.
    fn enter(&mut self, frames: &[ScopeFrame]) {
        let id = self.resolution_of(frames);
        self.current.push(id);
    }

    /// The id of what `frames` resolves.
    fn resolution_of(&mut self, frames: &[ScopeFrame]) -> u32 {
        let stack: Vec<_> = frames.iter().map(ScopeFrame::identity).collect();
        if let Some(&id) = self.stacks.get(&stack) {
            return id;
        }
        let next = self.resolutions.len() as u32;
        let id = *self
            .resolutions
            .entry(scope::resolution(frames))
            .or_insert(next);
        self.stacks.insert(stack, id);
        id
    }

    fn walk(
        &mut self,
        pattern: &Pattern,
        frames: &mut Vec<ScopeFrame>,
    ) -> Result<(), PatternError> {
        match pattern {
            Pattern::Reference(reference) => self.follow(reference, frames),
            Pattern::Context(context) => {
                frames.push(context.frame());
                self.enter(frames);
                let result = self.walk(context.child(), frames);
                self.current.pop();
                frames.pop();
                result
            }
            _ => pattern
                .child_patterns()
                .try_for_each(|child| self.walk(child, frames)),
        }
    }

    fn follow(&mut self, reference: &Reference, frames: &[ScopeFrame]) -> Result<(), PatternError> {
        let (master, key, home) = match reference.master() {
            Some(master) => (
                Rc::clone(master),
                TargetKey::Entry(Rc::as_ptr(master)),
                reference.home().cloned(),
            ),
            None => {
                let found =
                    scope::lookup(frames, reference.target()).ok_or_else(|| reference.unknown())?;
                (found.master, found.key, found.home)
            }
        };

        // A target with a home parses under its home's scopes, wherever the
        // reference sits (`scope::Found::home`), so walk it under those.
        let (frames, resolution) = match home {
            Some(home) if scope::home_applies(&home, frames) => {
                (home.to_vec(), self.resolution_of(&home))
            }
            _ => (
                frames.to_vec(),
                *self.current.last().expect("entered at the state's start"),
            ),
        };
        if self.seen.insert((key, resolution)) {
            // Walk what the parse would run: the master under the
            // reference's name (a renamed context answers to that name).
            self.pending.push(State {
                pattern: master.renamed_to(reference.meta.name),
                frames,
            });
        }
        Ok(())
    }
}

/// The root tree's first reference, depth first, children in order, not
/// inside references. Always the same one for the same root, so the cache
/// is found where it was stored.
fn first_reference(pattern: &Pattern) -> Option<&Reference> {
    match pattern {
        Pattern::Reference(reference) => Some(reference),
        _ => pattern
            .child_patterns()
            .into_iter()
            .find_map(first_reference),
    }
}

fn first_reference_mut(pattern: &mut Pattern) -> Option<&mut Reference> {
    match pattern {
        Pattern::Reference(reference) => Some(reference),
        _ => pattern.child_patterns_mut().find_map(first_reference_mut),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patterns::{Literal, Sequence};

    /// `a -> b -> missing`: the unknown name is two references away.
    fn chain() -> Pattern {
        let seq = |name: &str, next: &str| -> Pattern {
            Sequence::new(
                name,
                vec![
                    Literal::new("x", "x").unwrap().into(),
                    Reference::new(next).into(),
                ],
            )
            .unwrap()
            .into()
        };
        crate::patterns::Context::new(
            "root",
            Reference::new("a").into(),
            vec![seq("a", "b"), seq("b", "missing")],
        )
        .into()
    }

    fn check_within(root: &Pattern, budget: usize) -> Result<(), PatternError> {
        check_with(root, &Rc::new(Scope::implicit(root)), budget)
    }

    #[test]
    fn a_check_within_budget_finds_the_unknown_name() {
        assert!(check_within(&chain(), 3).is_err());
    }

    #[test]
    fn a_check_out_of_budget_leaves_the_rest_to_the_parse() {
        // Past the budget the check stops and passes; the parse then halts
        // when it reaches the name, as v12 throws then.
        let root = chain();
        assert_eq!(check_within(&root, 1), Ok(()));

        let mut root = root;
        let mut cursor = crate::Cursor::new("xxx");
        cursor.push_scope(ScopeFrame::new(
            Rc::new(Scope::implicit(&root)),
            root.meta(),
        ));
        assert!(root.parse(&mut cursor).is_none());
        assert!(matches!(
            cursor.pattern_error(),
            Some(PatternError::UnknownReference { target, .. }) if target == "missing"
        ));
    }
}
