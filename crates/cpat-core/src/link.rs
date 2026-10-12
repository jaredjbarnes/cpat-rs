//! Readies a root pattern's references before `exec` parses it: builds the
//! implicit scope from the root's tree and checks that every reference the
//! parse could reach names something (DESIGN §3).

use std::collections::HashSet;
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
    let key = (root.id(), root.meta().name);

    let Some(first) = first_reference(root) else {
        return Ok(None);
    };
    if let Some(link) = first.root_link().filter(|link| link.root == key) {
        return Ok(Some(Rc::clone(&link.scope)));
    }

    let scope = Rc::new(Scope::implicit(root));
    check_with(root, &scope)?;

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
pub(crate) fn check(root: &Pattern) -> Result<(), PatternError> {
    check_with(root, &Rc::new(Scope::implicit(root)))
}

fn check_with(root: &Pattern, scope: &Rc<Scope>) -> Result<(), PatternError> {
    let mut checker = Checker {
        frames: vec![ScopeFrame::new(Rc::clone(scope), root.meta())],
        walked: HashSet::new(),
    };
    checker.walk(root)
}

/// Walks the pattern the way a parse could, pushing a context's scope while
/// inside it and following each reference into what it would resolve to,
/// with the scope stack it would have then. A target already walked under
/// the same effective scopes is skipped, which ends recursive grammars.
struct Checker {
    frames: Vec<ScopeFrame>,
    walked: HashSet<(TargetKey, Vec<(*const Scope, Name)>)>,
}

impl Checker {
    fn walk(&mut self, pattern: &Pattern) -> Result<(), PatternError> {
        match pattern {
            Pattern::Reference(reference) => self.follow(reference),
            Pattern::Context(context) => {
                self.frames.push(context.frame());
                let result = self.walk(context.child());
                self.frames.pop();
                result
            }
            _ => pattern
                .child_patterns()
                .try_for_each(|child| self.walk(child)),
        }
    }

    fn follow(&mut self, reference: &Reference) -> Result<(), PatternError> {
        let (master, key) = match reference.master() {
            Some(master) => (Rc::clone(master), TargetKey::Entry(Rc::as_ptr(master))),
            None => {
                let found = scope::lookup(&self.frames, reference.target())
                    .ok_or_else(|| reference.unknown())?;
                (found.master, found.key)
            }
        };

        if !self.walked.insert((key, scope::effective(&self.frames))) {
            return Ok(());
        }

        // Walk what the parse would run: the master under the reference's
        // name (a renamed context answers to that name inside).
        self.walk(&master.renamed_to(reference.meta.name))
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
