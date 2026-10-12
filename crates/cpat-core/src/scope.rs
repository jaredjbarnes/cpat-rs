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
    // Shared: a grammar's rules all put the same definitions in scope, so
    // their contexts hold one table instead of one copy each (`Context::
    // sharing`).
    patterns: Rc<Definitions>,
    owner: Option<Owner>,
}

/// Master definitions by name, as a scope holds them.
pub(crate) type Definitions = HashMap<Name, Entry>;

/// One definition. `home` is where an implicit-scope target was found in
/// the root's tree: the contexts around it, outermost first. A target with
/// a home resolves its own references there, not where the reference that
/// found it sits (see `Found::home`). A context's definitions have none:
/// they are not in any tree, so they resolve wherever they are used.
/// Every reference inside the target uses the home, whatever wraps it;
/// v12 drops it for references wrapped beneath a choice (its choice clones
/// from never-parsed originals), a deviation recorded in DESIGN §11.
#[derive(Debug)]
pub(crate) struct Entry {
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
    target: FoundTarget,
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

/// What a lookup found, before it is needed as a pattern.
enum FoundTarget {
    Master(Rc<Pattern>),
    /// The context that owns the frame, answering to its own name. Built
    /// only when asked for (`Found::master`): it copies the context's whole
    /// child, and most lookups only want the key (the static check walks a
    /// target once however many references name it; a grammar rule's own
    /// name is named all over a big grammar).
    OwnContext(ScopeFrame),
}

impl Found {
    /// The master definition the lookup found.
    pub(crate) fn master(self) -> Rc<Pattern> {
        match self.target {
            FoundTarget::Master(master) => master,
            FoundTarget::OwnContext(frame) => {
                let own = frame
                    .scope
                    .owner
                    .as_ref()
                    .expect("an own-context hit has an owner");
                let context =
                    Context::from_scope(frame.owner, own.child.clone(), Rc::clone(&frame.scope));
                Rc::new(context.into())
            }
        }
    }
}

/// What a scope answers for a name, borrowed from it.
pub(crate) enum TargetRef<'a> {
    /// A definition, and where it sits when it came from the implicit scope.
    Entry {
        master: &'a Rc<Pattern>,
        home: Option<&'a Rc<[ScopeFrame]>>,
    },
    /// The context that owns the scope, answering to its own name: `child`
    /// is a pristine copy of its child (a scope can't hold the context that
    /// holds it).
    Owner {
        scope: &'a Scope,
        owner: Meta,
        child: &'a Pattern,
    },
}

impl Scope {
    /// A context's scope. Later patterns win over earlier ones with the same
    /// name, as v12's record assignment does.
    pub(crate) fn for_context(
        original_name: Name,
        child: Pattern,
        patterns: Vec<Pattern>,
    ) -> Scope {
        Scope::sharing(original_name, child, Rc::new(definitions(patterns)))
    }

    /// A context's scope over a table other contexts share.
    pub(crate) fn sharing(original_name: Name, child: Pattern, patterns: Rc<Definitions>) -> Scope {
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
            patterns: Rc::new(patterns),
            owner: None,
        }
    }

    /// What this scope answers for `name` when it sits on the stack as
    /// `owner`'s scope, borrowed: a definition, or the context itself
    /// (v12 `getPatternWithinContext`). `None` means "look further out".
    pub(crate) fn find_ref(&self, owner: Meta, name: Name) -> Option<TargetRef<'_>> {
        if let Some(own) = &self.owner
            && (name == owner.name || name == own.original_name)
        {
            // The context itself. If it only wraps a reference, v12 moves
            // on to the next context out without checking this one's list.
            if matches!(own.child, Pattern::Reference(_)) {
                return None;
            }
            return Some(TargetRef::Owner {
                scope: self,
                owner,
                child: &own.child,
            });
        }

        let entry = self.patterns.get(&name)?;
        is_valid_target(&entry.master).then_some(TargetRef::Entry {
            master: &entry.master,
            home: entry.home.as_ref(),
        })
    }

    pub(crate) fn patterns(&self) -> impl Iterator<Item = (Name, &Pattern)> {
        self.patterns
            .iter()
            .map(|(name, entry)| (*name, entry.master.as_ref()))
    }
}

/// Definitions of `patterns`, keyed by each pattern's own name. Later
/// patterns win over earlier ones with the same name, as v12's record
/// assignment does. They have no home: they sit in no tree.
pub(crate) fn definitions(patterns: Vec<Pattern>) -> Definitions {
    patterns
        .into_iter()
        .map(|pattern| {
            let entry = Entry {
                master: Rc::new(pattern),
                home: None,
            };
            (entry.master.meta().name, entry)
        })
        .collect()
}

/// `contexts` are the contexts around `pattern` in the root's tree, which
/// become each entry's home.
fn collect_first_matches<'r>(
    pattern: &'r Pattern,
    contexts: &mut Vec<&'r Context>,
    into: &mut HashMap<Name, Entry>,
) {
    visit_targets(pattern, contexts, &mut |target, contexts| {
        into.entry(target.meta().name).or_insert_with(|| Entry {
            master: Rc::new(target.clone()),
            home: Some(contexts.iter().map(|context| context.frame()).collect()),
        });
    });
}

/// Calls `visit` with every valid target in `pattern`'s tree, in v12
/// `findPattern` order (depth first, children before their parent, never
/// into a reference), with the contexts around it, outermost first. The
/// first target visited under a name is the one a search for it finds:
/// `exec`'s implicit scope and introspection's live lookup both read it.
pub(crate) fn visit_targets<'r>(
    pattern: &'r Pattern,
    contexts: &mut Vec<&'r Context>,
    visit: &mut dyn FnMut(&'r Pattern, &[&'r Context]),
) {
    if !matches!(pattern, Pattern::Reference(_)) {
        let entered = match pattern {
            Pattern::Context(context) => {
                contexts.push(context);
                true
            }
            _ => false,
        };
        for child in pattern.child_patterns() {
            visit_targets(child, contexts, visit);
        }
        if entered {
            contexts.pop();
        }
    }

    if is_valid_target(pattern) {
        visit(pattern, contexts);
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
        Some(match self.scope.find_ref(self.owner, name)? {
            // No home: v12 clones the live context, but a context's clone
            // clones its child twice and the second copy drops what its
            // references resolved, so they resolve where they are used.
            TargetRef::Owner { scope, .. } => Found {
                target: FoundTarget::OwnContext(self.clone()),
                key: TargetKey::Owner(std::ptr::from_ref(scope)),
                home: None,
            },
            TargetRef::Entry { master, home } => Found {
                target: FoundTarget::Master(Rc::clone(master)),
                key: TargetKey::Entry(Rc::as_ptr(master)),
                home: home.map(Rc::clone),
            },
        })
    }

    pub(crate) fn scope(&self) -> &Scope {
        &self.scope
    }

    pub(crate) fn owner(&self) -> Meta {
        self.owner
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
