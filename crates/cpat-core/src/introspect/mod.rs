//! Introspection (v12's `getTokens`, `getPatterns`, `…After`, `getNext…`,
//! `find`): what can start a pattern, what may follow one, and searches,
//! for autocomplete and error messages. DESIGN §16.
//!
//! v12 answers these by walking `parent` pointers. Rust patterns have none
//! (DESIGN §2), so every question is asked about a place, a `PatternPath`
//! from a root, and the answer names places too (`Handle`), so it can be
//! asked about in turn.
//!
//! ```
//! use cpat_core::{Literal, Optional, Pattern, PatternPath, Sequence};
//!
//! let greeting: Pattern = Sequence::new("greeting", vec![
//!     Literal::new("hello", "Hello").unwrap().into(),
//!     Optional::new("comma", Literal::new(",", ",").unwrap().into()).into(),
//!     Literal::new("world", " World").unwrap().into(),
//! ]).unwrap().into();
//!
//! assert_eq!(greeting.tokens(), ["Hello"]);
//! let hello = greeting.find(|p| p.name() == "hello").unwrap();
//! assert_eq!(hello.path, PatternPath::new(&[0]));
//! assert_eq!(greeting.next_tokens(&hello.path), [",", " World"]);
//! ```

mod find;
mod frames;
mod loc;
mod pattern_api;
mod select;

use std::cell::Cell;
use std::rc::Rc;

use crate::halt::Limits;
use crate::name::Name;
use crate::pattern::Pattern;
use crate::pattern_error::PatternError;
use crate::pattern_id::{Meta, PatternId};
use crate::pattern_path::PatternPath;
use crate::patterns::ChoiceRole;

use frames::{FrameRef, LiveScope};
use loc::Loc;
pub(crate) use select::leaf_tokens;
use select::{Patterns, Tokens};

/// Questions about the patterns in `root`'s tree, each about a place in it.
///
/// References are followed as a parse would follow them: through the
/// contexts around them, then the root's own tree (DESIGN §3, §11), or to
/// what a parse already resolved them to. A name that resolves to nothing
/// contributes nothing (v12 throws); `Pattern::check` reports it.
pub struct Introspection<'r> {
    root: &'r Pattern,
    /// The implicit scope: the one an `exec` readied for this root, so
    /// references it resolved keep their homes, or else the live tree.
    bottom: FrameRef<'r>,
    live: LiveScope<'r>,
    /// Places made by the query running now (`loc::BUDGET`).
    spent: Cell<usize>,
    /// The depth of the place the query running now was asked about; the
    /// query expands at most `loc::MAX_DEPTH` below it.
    floor: Cell<usize>,
    /// The longest path a query follows (`Limits::max_depth`): no parse
    /// under the same limits makes a longer one, and a longer path is
    /// refused rather than walked (`Loc::walk`).
    max_path: usize,
}

/// A pattern instance at a place in the root's tree: what a query found,
/// and the path to ask about it next.
#[derive(Clone, Debug)]
pub struct Handle<'r> {
    /// The child indices from the root (`Pattern::at_path` numbering).
    pub path: PatternPath,
    /// The pattern there. A reference's target is the definition it found,
    /// which a parse would clone and rename; `name` is that clone's name.
    pub pattern: &'r Pattern,
    name: Name,
    id: Option<PatternId>,
}

impl Handle<'_> {
    /// The name the instance here answers to: a reference's target is named
    /// after the reference, as v12's resolved clone is.
    pub fn name(&self) -> &'static str {
        self.name.as_str()
    }

    /// The instance's id, shared by its clones. `None` for a choice's
    /// operator, which v12 mints a fresh id for.
    pub fn id(&self) -> Option<PatternId> {
        self.id
    }
}

impl<'r> Introspection<'r> {
    /// Introspection that follows paths as deep as a parse under
    /// `Limits::default()` makes them (`exec`'s paths).
    pub fn new(root: &'r Pattern) -> Introspection<'r> {
        Introspection::with_limits(root, Limits::default())
    }

    /// Introspection that follows paths as deep as a parse under `limits`
    /// makes them (`exec_with`'s paths). A longer path leads nowhere. Like
    /// the parse, a query this deep needs the stack that `limits` asks for.
    pub fn with_limits(root: &'r Pattern, limits: Limits) -> Introspection<'r> {
        let bottom = match crate::link::readied_scope_ref(root) {
            Some(scope) => FrameRef::Scope {
                scope,
                owner: Meta {
                    id: root.id(),
                    name: root.meta().name,
                },
            },
            None => FrameRef::Live,
        };
        Introspection {
            root,
            bottom,
            live: LiveScope::new(root),
            spent: Cell::new(0),
            floor: Cell::new(0),
            max_path: limits.max_depth,
        }
    }

    /// The instance `path` leads to. Unlike `Pattern::at_path`, a
    /// reference that hasn't parsed yet is followed to what it would
    /// resolve to.
    pub fn at(&self, path: &PatternPath) -> Option<Handle<'r>> {
        self.start().walk(path).map(|loc| handle(&loc))
    }

    /// What can start the pattern at `path` (v12 `getTokens`).
    pub fn tokens(&self, path: &PatternPath) -> Vec<String> {
        self.start()
            .walk(path)
            .map(|loc| self.leading::<Tokens>(&loc))
            .unwrap_or_default()
    }

    /// The leaf patterns that can start the pattern at `path` (v12
    /// `getPatterns`).
    pub fn patterns(&self, path: &PatternPath) -> Vec<Handle<'r>> {
        self.start()
            .walk(path)
            .map(|loc| handles(&self.leading::<Patterns>(&loc)))
            .unwrap_or_default()
    }

    /// The tokens that may follow child `child` of the pattern at `path`
    /// (v12 `getTokensAfter`). Children are numbered as in
    /// `Pattern::at_path`.
    pub fn tokens_after(&self, path: &PatternPath, child: usize) -> Vec<String> {
        self.start()
            .walk(path)
            .map(|loc| self.after::<Tokens>(&loc, child))
            .unwrap_or_default()
    }

    /// The patterns that may follow child `child` of the pattern at `path`
    /// (v12 `getPatternsAfter`).
    pub fn patterns_after(&self, path: &PatternPath, child: usize) -> Vec<Handle<'r>> {
        self.start()
            .walk(path)
            .map(|loc| handles(&self.after::<Patterns>(&loc, child)))
            .unwrap_or_default()
    }

    /// The tokens that may follow the pattern at `path` in its parent (v12
    /// `getNextTokens`): nothing at the root.
    pub fn next_tokens(&self, path: &PatternPath) -> Vec<String> {
        self.start()
            .walk(path)
            .map(|loc| self.next::<Tokens>(&loc))
            .unwrap_or_default()
    }

    /// The patterns that may follow the pattern at `path` in its parent
    /// (v12 `getNextPatterns`).
    pub fn next_patterns(&self, path: &PatternPath) -> Vec<Handle<'r>> {
        self.start()
            .walk(path)
            .map(|loc| handles(&self.next::<Patterns>(&loc)))
            .unwrap_or_default()
    }

    /// The first pattern `predicate` accepts, as the root's v12 `find`
    /// searches: depth first, children before their parent, never into a
    /// reference.
    pub fn find(&self, predicate: impl Fn(&Pattern) -> bool) -> Option<Handle<'r>> {
        self.start()
            .find_from(&self.root_loc(), &predicate)
            .map(|loc| handle(&loc))
    }

    /// v12's `Expression.build()`: classifying the choice at `path`
    /// resolves each alternative that is a reference, and one that names
    /// nothing is an error (v12 throws). `Ok` for any other place, and for
    /// a reference deeper inside an alternative: `Pattern::check` and
    /// `exec` report those.
    pub fn check_alternatives(&self, path: &PatternPath) -> Result<(), PatternError> {
        match self.start().walk(path) {
            Some(loc) => self.unresolved_alternative(&loc).map_or(Ok(()), Err),
            None => Ok(()),
        }
    }

    /// What each child of the choice at `path` is, in `Pattern::at_path`
    /// numbering (v12's `atomPatterns`, `prefixPatterns`, `postfixPatterns`
    /// and `infixPatterns` partition the children by it); `None` when the
    /// place isn't a choice. A choice that waits for its first parse to be
    /// classified (a reference alternative) is classified here as that
    /// parse would classify it, as v12's `build()` does on demand.
    pub fn choice_roles(&self, path: &PatternPath) -> Option<Vec<ChoiceRole>> {
        let loc = self.start().walk(path)?;
        self.roles(&loc)
    }

    /// Starts a query: it gets the whole budget, and the depth limit counts
    /// from the root until a walk moves it.
    fn start(&self) -> &Self {
        self.spent.set(0);
        self.floor.set(0);
        self
    }

    fn root_loc(&self) -> Rc<Loc<'r>> {
        Loc::root(self.root, self.bottom)
    }
}

fn handle<'r>(loc: &Loc<'r>) -> Handle<'r> {
    Handle {
        path: loc.path(),
        pattern: loc.pattern(),
        name: loc.name,
        id: loc.id,
    }
}

fn handles<'r>(locs: &[Rc<Loc<'r>>]) -> Vec<Handle<'r>> {
    locs.iter().map(|loc| handle(loc)).collect()
}
