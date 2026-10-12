use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use crate::name::Name;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
use crate::patterns::Context;

/// Master definitions a `Reference` can resolve to, by name (DESIGN §3).
///
/// A `Context` owns one, listing the patterns it was given. `exec` builds an
/// implicit one from the root pattern's own tree. Masters are never parsed:
/// a reference clones the master it finds and parses the clone, so every
/// recursion level gets its own instance.
#[derive(Debug)]
pub(crate) struct Scope {
    patterns: HashMap<Name, Rc<Pattern>>,
    owner: Option<Owner>,
}

/// A context answers to its own name with itself (v12
/// `getPatternWithinContext`). A scope can't hold the context that holds it
/// (that would be an `Rc` cycle), so it keeps what is needed to make one:
/// the name the context was built with and a pristine copy of its child.
#[derive(Debug)]
struct Owner {
    original_name: Name,
    child: Pattern,
}

/// A scope as it sits on the cursor: the scope plus, for a context's scope,
/// the context instance it belongs to (its id and current name).
#[derive(Clone, Debug)]
pub(crate) struct ScopeFrame {
    scope: Rc<Scope>,
    owner: Meta,
}

/// Identifies what a lookup found, so the static check can tell a target it
/// has already walked from a new one. An own-context hit builds a fresh
/// pattern each time, so it is keyed by its scope instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum TargetKey {
    Entry(*const Pattern),
    Owner(*const Scope),
}

pub(crate) struct Found {
    pub(crate) master: Rc<Pattern>,
    pub(crate) key: TargetKey,
}

impl Scope {
    /// A context's scope. Later patterns win over earlier ones with the same
    /// name, as v12's record assignment does.
    pub(crate) fn for_context(
        original_name: Name,
        child: Pattern,
        patterns: Vec<Pattern>,
    ) -> Scope {
        let patterns = patterns
            .into_iter()
            .map(|pattern| (pattern.meta().name, Rc::new(pattern)))
            .collect();
        Scope {
            patterns,
            owner: Some(Owner {
                original_name,
                child,
            }),
        }
    }

    /// The implicit scope `exec` builds from the root's tree: v12's
    /// `findPattern(root, …)`. That search is depth first, children before
    /// their parent, never into a reference, and the first valid match wins;
    /// building the whole table in that order and keeping the first entry per
    /// name answers every later search the same way.
    pub(crate) fn implicit(root: &Pattern) -> Scope {
        let mut patterns = HashMap::new();
        collect_first_matches(root, &mut patterns);
        Scope {
            patterns,
            owner: None,
        }
    }

    pub(crate) fn patterns(&self) -> impl Iterator<Item = (Name, &Pattern)> {
        self.patterns.iter().map(|(name, p)| (*name, p.as_ref()))
    }
}

fn collect_first_matches(pattern: &Pattern, into: &mut HashMap<Name, Rc<Pattern>>) {
    if !matches!(pattern, Pattern::Reference(_)) {
        for child in pattern.child_patterns() {
            collect_first_matches(child, into);
        }
    }

    if is_valid_target(pattern) {
        into.entry(pattern.meta().name)
            .or_insert_with(|| Rc::new(pattern.clone()));
    }
}

/// v12 `_isValidPattern`: a reference never resolves to another reference,
/// nor to a context that only wraps one. Either would resolve to nothing of
/// its own.
pub(crate) fn is_valid_target(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Reference(_) => false,
        Pattern::Context(context) => !matches!(context.child(), Pattern::Reference(_)),
        _ => true,
    }
}

impl ScopeFrame {
    pub(crate) fn new(scope: Rc<Scope>, owner: Meta) -> ScopeFrame {
        ScopeFrame { scope, owner }
    }

    /// What this frame answers for `name`. `None` means "look further out":
    /// the name is absent, or what it names isn't a valid target.
    fn find(&self, name: Name) -> Option<Found> {
        if let Some(owner) = &self.scope.owner
            && (name == self.owner.name || name == owner.original_name)
        {
            // The context itself. If it only wraps a reference, v12 moves
            // on to the next context out without checking this one's list.
            if matches!(owner.child, Pattern::Reference(_)) {
                return None;
            }
            let context =
                Context::from_scope(self.owner, owner.child.clone(), Rc::clone(&self.scope));
            return Some(Found {
                master: Rc::new(context.into()),
                key: TargetKey::Owner(Rc::as_ptr(&self.scope)),
            });
        }

        let master = self.scope.patterns.get(&name)?;
        is_valid_target(master).then(|| Found {
            master: Rc::clone(master),
            key: TargetKey::Entry(Rc::as_ptr(master)),
        })
    }

    /// Which frame this is: frames with the same scope and owner name
    /// answer every lookup alike.
    pub(crate) fn identity(&self) -> (*const Scope, Name) {
        (Rc::as_ptr(&self.scope), self.owner.name)
    }

    /// Every name this frame might answer: its definitions and, for a
    /// context's scope, the context's own names.
    fn names(&self) -> impl Iterator<Item = Name> + '_ {
        let own = self
            .scope
            .owner
            .as_ref()
            .map(|owner| [self.owner.name, owner.original_name]);
        self.scope
            .patterns
            .keys()
            .copied()
            .chain(own.into_iter().flatten())
    }
}

/// Looks `name` up from the innermost frame out, as v12 walks its parent
/// chain for contexts and then searches the root.
pub(crate) fn lookup(frames: &[ScopeFrame], name: Name) -> Option<Found> {
    frames.iter().rev().find_map(|frame| frame.find(name))
}

/// Everything `frames` resolves, as `(name, target)` sorted by name: what
/// a lookup would find for each name any frame answers. Two stacks with the
/// same resolution behave alike for every later lookup, whatever order their
/// frames came in, which is what lets the static check stop on cycles
/// without telling apart stacks that only differ in order (that would grow
/// with the number of orderings of nested contexts).
pub(crate) fn resolution(frames: &[ScopeFrame]) -> Vec<(Name, TargetKey)> {
    let mut decided: HashSet<Name> = HashSet::new();
    let mut resolved = Vec::new();
    for frame in frames.iter().rev() {
        for name in frame.names() {
            if decided.contains(&name) {
                continue;
            }
            // A frame that declines a name (an invalid target) leaves it to
            // the frames further out, exactly as `lookup` does.
            if let Some(found) = frame.find(name) {
                decided.insert(name);
                resolved.push((name, found.key));
            }
        }
    }
    resolved.sort_unstable();
    resolved
}
