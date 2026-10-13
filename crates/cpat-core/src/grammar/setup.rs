//! What one build is given: the caller's options, and for a grammar built
//! on behalf of another (an import, a `with params` block) what it shares
//! with that one.

use std::collections::HashSet;
use std::rc::Rc;

use crate::pattern::Pattern;

use super::anonymous::AnonymousNames;
use super::decorators::Decorators;
use super::imports::ImportResolver;
use super::options::GrammarOptions;

pub(super) struct Setup {
    pub(super) resolver: Option<Rc<dyn ImportResolver>>,
    /// The resource this grammar's text came from, if known.
    pub(super) origin: Option<String>,
    pub(super) params: Vec<Pattern>,
    /// The caller's decorators merged with the built-in ones.
    pub(super) decorators: Rc<Decorators>,
    pub(super) anonymous: AnonymousNames,
    /// Names the grammar may use that the grammar enclosing it defines: a
    /// `with params` block names the importer's rules, which are built
    /// after its imports and so can't be params yet (v12 `_outerNames`).
    pub(super) outer_names: HashSet<String>,
    /// The resources of the grammars importing this one, outermost first.
    pub(super) importers: Vec<String>,
}

impl Setup {
    /// A build of the caller's own text, with a count of its own.
    pub(super) fn top_level(options: GrammarOptions) -> Setup {
        let GrammarOptions {
            resolver,
            origin_resource,
            params,
            decorators,
        } = options;
        Setup {
            resolver,
            // v12 passes `originResource || null`: an empty name is none.
            origin: origin_resource.filter(|origin| !origin.is_empty()),
            params,
            decorators: Rc::new(decorators.with_built_ins()),
            anonymous: AnonymousNames::default(),
            outer_names: HashSet::new(),
            importers: Vec::new(),
        }
    }

    /// The build of a grammar this one imports from `resource`.
    pub(super) fn for_import(&self, resource: &str, params: Vec<Pattern>) -> Setup {
        let mut importers = self.importers.clone();
        importers.extend(self.origin.clone());
        Setup {
            resolver: self.resolver.clone(),
            origin: Some(resource.to_string()).filter(|origin| !origin.is_empty()),
            params,
            decorators: Rc::clone(&self.decorators),
            anonymous: self.anonymous.shared(),
            outer_names: HashSet::new(),
            importers,
        }
    }

    /// The build of a `with params { … }` block: it sees `params` (the
    /// importer's imports and params) and may name `outer_names`.
    pub(super) fn for_with_params(
        &self,
        params: Vec<Pattern>,
        outer_names: HashSet<String>,
    ) -> Setup {
        Setup {
            resolver: self.resolver.clone(),
            origin: self.origin.clone(),
            params,
            decorators: Rc::clone(&self.decorators),
            anonymous: self.anonymous.shared(),
            outer_names,
            importers: self.importers.clone(),
        }
    }
}
