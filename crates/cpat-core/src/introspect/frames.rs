//! Name lookup for introspection: the parse's scope rules (DESIGN §3, §11),
//! borrowed from the pattern tree instead of cloned, so what a query finds
//! can be handed back with the root's lifetime.

use std::cell::OnceCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::name::Name;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
use crate::patterns::{Context, Reference};
use crate::scope::{self, Scope, ScopeFrame, TargetRef};

/// One scope on the stack, as the cursor would hold it while parsing here.
#[derive(Clone, Copy)]
pub(super) enum FrameRef<'r> {
    /// A context's definitions, or the implicit scope an `exec` readied
    /// for this root.
    Scope { scope: &'r Scope, owner: Meta },
    /// The root's own tree, searched as v12's `findPattern(root)`: the
    /// implicit scope of a root that was never exec'd. Looked up in the
    /// live tree rather than built, because `exec`'s scope holds clones,
    /// and a clone can't be handed back with the root's lifetime.
    Live,
}

impl<'r> FrameRef<'r> {
    /// A context's frame, the context answering to `name` (its own, or the
    /// reference's that renamed it).
    pub(super) fn of_context(context: &'r Context, name: Name) -> FrameRef<'r> {
        FrameRef::Scope {
            scope: context.scope(),
            owner: Meta {
                id: context.meta.id,
                name,
            },
        }
    }

    fn of(frame: &'r ScopeFrame) -> FrameRef<'r> {
        FrameRef::Scope {
            scope: frame.scope(),
            owner: frame.owner(),
        }
    }

    fn is(&self, other: &FrameRef<'_>) -> bool {
        match (self, other) {
            (FrameRef::Scope { scope: a, .. }, FrameRef::Scope { scope: b, .. }) => {
                std::ptr::eq(*a, *b)
            }
            (FrameRef::Live, FrameRef::Live) => true,
            _ => false,
        }
    }
}

pub(super) type Frames<'r> = Rc<[FrameRef<'r>]>;

/// What a name resolved to.
#[derive(Clone, Copy)]
pub(super) enum Target<'r> {
    Pattern(&'r Pattern),
    /// A context answering to its own name: its scope and a pristine copy
    /// of its child (`scope::TargetRef::Owner`).
    OwnContext {
        scope: &'r Scope,
        owner: Meta,
        child: &'r Pattern,
    },
}

/// The root's tree as a table: each name's first valid target in v12
/// `findPattern` order, with the contexts around it (its home). Built on
/// the first lookup that reaches it: most queries never meet a reference.
pub(super) struct LiveScope<'r> {
    root: &'r Pattern,
    targets: OnceCell<HashMap<Name, Home<'r>>>,
}

type Home<'r> = (&'r Pattern, Vec<&'r Context>);

impl<'r> LiveScope<'r> {
    pub(super) fn new(root: &'r Pattern) -> LiveScope<'r> {
        LiveScope {
            root,
            targets: OnceCell::new(),
        }
    }

    fn get(&self, name: Name) -> Option<&Home<'r>> {
        let targets = self.targets.get_or_init(|| {
            let mut targets = HashMap::new();
            scope::visit_targets(self.root, &mut Vec::new(), &mut |target, contexts| {
                targets
                    .entry(target.meta().name)
                    .or_insert_with(|| (target, contexts.to_vec()));
            });
            targets
        });
        targets.get(&name)
    }
}

/// Looks `name` up from the innermost frame out, as the cursor does
/// (`scope::lookup`). A target with a home takes the frames up to the one
/// that answered plus its home's contexts, as the parse does.
pub(super) fn lookup<'r>(
    frames: &Frames<'r>,
    name: Name,
    live: &LiveScope<'r>,
) -> Option<(Target<'r>, Frames<'r>)> {
    for (at, frame) in frames.iter().enumerate().rev() {
        let below = || frames[..=at].iter().copied();
        match frame {
            FrameRef::Scope { scope, owner } => match scope.find_ref(*owner, name) {
                None => continue,
                Some(TargetRef::Owner {
                    scope,
                    owner,
                    child,
                }) => {
                    // No home: see `ScopeFrame::find`.
                    let target = Target::OwnContext {
                        scope,
                        owner,
                        child,
                    };
                    return Some((target, Rc::clone(frames)));
                }
                Some(TargetRef::Entry { master, home }) => {
                    let frames = match home {
                        Some(home) => below().chain(home.iter().map(FrameRef::of)).collect(),
                        None => Rc::clone(frames),
                    };
                    return Some((Target::Pattern(master), frames));
                }
            },
            FrameRef::Live => {
                let Some((target, contexts)) = live.get(name) else {
                    continue;
                };
                let home = contexts
                    .iter()
                    .map(|context| FrameRef::of_context(context, context.meta.name));
                return Some((Target::Pattern(target), below().chain(home).collect()));
            }
        }
    }
    None
}

/// What `reference` parses at a place whose frames are `frames`, and under
/// which frames: its instance or master when a parse already resolved it
/// (v12 `_pattern`, `_cachedPattern`), with their home when that home
/// belongs to the scope in force here (`scope::home_applies`); otherwise a
/// lookup of its name.
pub(super) fn resolve<'r>(
    reference: &'r Reference,
    frames: &Frames<'r>,
    live: &LiveScope<'r>,
) -> Option<(Target<'r>, Frames<'r>)> {
    let known = reference
        .instance()
        .or_else(|| reference.master().map(|master| &**master));
    let Some(known) = known else {
        return lookup(frames, reference.target(), live);
    };
    let frames = match reference.home() {
        Some(home) if applies(home, frames) => home.iter().map(FrameRef::of).collect(),
        _ => Rc::clone(frames),
    };
    Some((Target::Pattern(known), frames))
}

/// `scope::home_applies` for borrowed frames: the home was found in the
/// implicit scope that is the bottom frame here.
fn applies(home: &[ScopeFrame], frames: &[FrameRef<'_>]) -> bool {
    match (home.first(), frames.first()) {
        (Some(home), Some(here)) => FrameRef::of(home).is(here),
        _ => false,
    }
}
