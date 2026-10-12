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
    patterns: HashMap<Name, Entry>,
    owner: Option<Owner>,
}

/// One definition. `home` is where an implicit-scope target was found in
/// the root's tree: the contexts around it, outermost first. A target with
/// a home resolves its own references there, not where the reference that
/// found it sits (see `Found::home`). A context's definitions have none:
/// they are not in any tree, so they resolve wherever they are used.
#[derive(Debug)]
struct Entry {
    master: Rc<Pattern>,
    home: Option<Rc<[ScopeFrame]>>,
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
    /// The whole scope stack the master parses under, when it has a home:
    /// the frames up to the one that answered (the implicit scope), then
    /// the contexts around the definition. `None`: it parses under the
    /// reference's own stack.
    ///
    /// Why: v12's implicit scope hands out the LIVE pattern in the root's
    /// tree, and cloning it keeps whatever its references already resolved
    /// to there. So once the definition has parsed, a reference from
    /// anywhere else sees the names of the definition's own contexts
    /// (`tests/reference.rs`,
    /// `an_implicit_target_resolves_names_where_it_is_defined`).
    pub(crate) home: Option<Rc<[ScopeFrame]>>,
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
            .map(|pattern| {
                let entry = Entry {
                    master: Rc::new(pattern),
                    home: None,
                };
                (entry.master.meta().name, entry)
            })
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
        collect_first_matches(root, &mut Vec::new(), &mut patterns);
        Scope {
            patterns,
            owner: None,
        }
    }

    pub(crate) fn patterns(&self) -> impl Iterator<Item = (Name, &Pattern)> {
        self.patterns
            .iter()
            .map(|(name, entry)| (*name, entry.master.as_ref()))
    }
}

/// `contexts` are the contexts around `pattern` in the root's tree, which
/// become each entry's home.
fn collect_first_matches(
    pattern: &Pattern,
    contexts: &mut Vec<ScopeFrame>,
    into: &mut HashMap<Name, Entry>,
) {
    if !matches!(pattern, Pattern::Reference(_)) {
        let context = match pattern {
            Pattern::Context(context) => Some(context.frame()),
            _ => None,
        };
        let entered = context.map(|frame| contexts.push(frame)).is_some();
        for child in pattern.child_patterns() {
            collect_first_matches(child, contexts, into);
        }
        if entered {
            contexts.pop();
        }
    }

    if is_valid_target(pattern) {
        into.entry(pattern.meta().name).or_insert_with(|| Entry {
            master: Rc::new(pattern.clone()),
            home: Some(contexts.as_slice().into()),
        });
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
    /// the name is absent, or what it names isn't a valid target. The
    /// `home` here holds only the definition's contexts; `lookup` adds the
    /// frames below.
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
            // No home: v12 clones the live context, but a context's
            // clone clones its child twice and the second copy drops what
            // its references resolved, so they resolve where they are used.
            return Some(Found {
                master: Rc::new(context.into()),
                key: TargetKey::Owner(Rc::as_ptr(&self.scope)),
                home: None,
            });
        }

        let entry = self.scope.patterns.get(&name)?;
        is_valid_target(&entry.master).then(|| Found {
            master: Rc::clone(&entry.master),
            key: TargetKey::Entry(Rc::as_ptr(&entry.master)),
            home: entry.home.clone(),
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

/// Whether a target's `home` still applies under the `visible` frames: it
/// was found in the same implicit scope (the bottom frame) as the one in
/// force now. A reference whose master and home were found in another root
/// (it was cloned out of that tree, or the root was renamed) has left that
/// home; its target resolves where it now sits, as v12 resolves whatever
/// its cached pattern had not resolved yet.
pub(crate) fn home_applies(home: &[ScopeFrame], visible: &[ScopeFrame]) -> bool {
    match (home.first(), visible.first()) {
        (Some(home), Some(here)) => Rc::ptr_eq(&home.scope, &here.scope),
        _ => false,
    }
}

/// Looks `name` up from the innermost frame out, as v12 walks its parent
/// chain for contexts and then searches the root.
pub(crate) fn lookup(frames: &[ScopeFrame], name: Name) -> Option<Found> {
    let (at, mut found) = frames
        .iter()
        .enumerate()
        .rev()
        .find_map(|(at, frame)| frame.find(name).map(|found| (at, found)))?;
    found.home = found.home.map(|contexts| {
        frames[..=at]
            .iter()
            .chain(contexts.iter())
            .cloned()
            .collect()
    });
    Some(found)
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
