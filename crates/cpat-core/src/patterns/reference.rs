use std::fmt;
use std::rc::Rc;

use crate::cursor::Cursor;
use crate::halt::Halt;
use crate::name::Name;
use crate::pattern::Pattern;
use crate::pattern_error::PatternError;
use crate::pattern_id::{Meta, PatternId};
use crate::scope::{Scope, ScopeFrame};
use crate::tree::NodeId;

/// A pattern named elsewhere: how grammars recurse.
///
/// The first time it parses, it looks its target name up on the cursor's
/// scope stack (DESIGN §3), clones the master it finds under its own name
/// and keeps that clone as its only child. The clone has the master's id, so
/// the recursion guard can recognise it. A clone of a `Reference` starts
/// unresolved again, which is how each recursion level gets a fresh
/// instance instead of re-entering a busy one (DESIGN §1).
pub struct Reference {
    pub(crate) meta: Meta,
    target: Name,
    // v12 `_cachedPattern`: once found, clones reuse the master instead of
    // searching again. Masters are immutable behind the `Rc`.
    master: Option<Rc<Pattern>>,
    // The scope stack the master parses under, when it came from the
    // implicit scope (see `scope::Found::home`). Kept with the master.
    home: Option<Rc<[ScopeFrame]>>,
    // v12 `_pattern`: this level's own instance.
    instance: Option<Box<Pattern>>,
    // The implicit root scope `exec` built and checked, cached here because
    // this is the root tree's first reference (see `link`).
    root_link: Option<RootLink>,
}

/// An implicit scope, and the root it was built for. A pattern that was
/// exec'd and then moved into a bigger one has a different root, so the
/// cached scope no longer applies.
#[derive(Clone, Debug)]
pub(crate) struct RootLink {
    pub(crate) root: (PatternId, Name),
    pub(crate) scope: Rc<Scope>,
}

impl Reference {
    /// A reference to the pattern called `name`.
    pub fn new(name: &str) -> Reference {
        Reference::to(name, name)
    }

    /// A reference called `name` to the pattern called `target`.
    pub fn to(name: &str, target: &str) -> Reference {
        Reference {
            meta: Meta::new(name),
            target: Name::new(target),
            master: None,
            home: None,
            instance: None,
            root_link: None,
        }
    }

    /// The name this reference looks up.
    pub fn target_name(&self) -> &'static str {
        self.target.as_str()
    }

    pub(crate) fn target(&self) -> Name {
        self.target
    }

    pub(crate) fn master(&self) -> Option<&Rc<Pattern>> {
        self.master.as_ref()
    }

    pub(crate) fn home(&self) -> Option<&Rc<[ScopeFrame]>> {
        self.home.as_ref()
    }

    pub(crate) fn root_link(&self) -> Option<&RootLink> {
        self.root_link.as_ref()
    }

    pub(crate) fn set_root_link(&mut self, link: RootLink) {
        self.root_link = Some(link);
    }

    pub(crate) fn unknown(&self) -> PatternError {
        PatternError::UnknownReference {
            name: self.meta.name.as_str().to_string(),
            target: self.target.as_str().to_string(),
        }
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let first = cursor.pos();

        if !self.resolve(cursor) {
            // Reachable when `parse` is driven directly, or when `exec`'s
            // up-front check gave up on a huge grammar (see `link`). v12
            // throws here; this halts the parse with the reason as data.
            let unknown = self.unknown();
            cursor.halt(first, self.meta, Halt::Pattern(unknown));
            return None;
        }
        let instance = self.instance.as_deref_mut().expect("resolved above");

        // v12 `_isBeyondRecursiveAllowance`: the target may already be
        // parsing at this very spot once (that is how `a = a "b" | "b"`
        // gets its one inner level), but a second time would never end.
        if cursor.ancestors_at(instance.id(), first, 2) > 1 {
            cursor.record_error(first, first, self.meta);
            return None;
        }

        match &self.home {
            Some(home) if cursor.home_applies(home) => {
                cursor.within_scopes(home, |cursor| instance.parse(cursor))
            }
            _ => instance.parse(cursor),
        }
    }

    /// Makes this level's instance if there is none yet. `false`: the
    /// target names nothing here.
    fn resolve(&mut self, cursor: &Cursor<'_>) -> bool {
        if self.instance.is_none() {
            if self.master.is_none() {
                let Some(found) = cursor.lookup(self.target) else {
                    return false;
                };
                self.master = Some(found.master);
                self.home = found.home;
            }
            let master = self.master.as_ref().expect("found above");
            self.instance = Some(Box::new(master.renamed_to(self.meta.name)));
        }
        true
    }
}

impl Clone for Reference {
    /// Unresolved, but sharing the master once one was found (v12 `clone`
    /// copies `_pattern` into `_cachedPattern`). Never the instance: the
    /// clone is another recursion level and must not share parse state.
    fn clone(&self) -> Reference {
        Reference {
            meta: self.meta,
            target: self.target,
            master: self.master.clone(),
            home: self.home.clone(),
            instance: None,
            root_link: None,
        }
    }
}

impl fmt::Debug for Reference {
    // Not derived: an instance holds the next recursion level, and printing
    // it would print the whole resolved chain.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reference")
            .field("meta", &self.meta)
            .field("target", &self.target)
            .field("resolved", &self.instance.is_some())
            .finish()
    }
}
