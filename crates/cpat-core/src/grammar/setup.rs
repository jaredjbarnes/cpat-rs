//! What one build is given: the caller's options, and for a grammar built
//! on behalf of another (an import, a `with params` block) what it shares
//! with that one.

use std::collections::HashSet;
use std::rc::Rc;

use crate::pattern::Pattern;

use super::anonymous::AnonymousNames;
use super::decorators::Decorators;
use super::imports::{GrammarFile, ImportResolver};
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
    /// The grammars being built, outermost first, ending with this one.
    pub(super) chain: Vec<Link>,
}

/// One grammar in a chain of imports. What a build does next depends only on
/// its text and the origin its own imports are resolved from, so a link that
/// repeats both is a cycle. The resource alone is not enough: a resolver may
/// answer different grammars under one name (v12's own tests do), or under
/// none.
#[derive(Clone)]
pub(super) struct Link {
    pub(super) resource: Option<String>,
    pub(super) expression: Rc<str>,
}

impl Link {
    fn new(resource: Option<String>, expression: &str) -> Link {
        Link {
            resource,
            expression: Rc::from(expression),
        }
    }

    /// Whether building `file` would repeat this link exactly.
    pub(super) fn repeats(&self, file: &GrammarFile) -> bool {
        self.resource == origin(&file.resource) && *self.expression == *file.expression
    }
}

/// v12 passes `originResource || null`: an empty name is none.
fn origin(resource: &str) -> Option<String> {
    Some(resource.to_string()).filter(|resource| !resource.is_empty())
}

impl Setup {
    /// A build of the caller's own text, with a count of its own.
    pub(super) fn top_level(text: &str, options: GrammarOptions) -> Setup {
        let GrammarOptions {
            resolver,
            origin_resource,
            params,
            decorators,
        } = options;
        let origin = origin_resource.as_deref().and_then(origin);
        Setup {
            resolver,
            chain: vec![Link::new(origin.clone(), text)],
            origin,
            params,
            decorators: Rc::new(decorators.with_built_ins()),
            anonymous: AnonymousNames::default(),
            outer_names: HashSet::new(),
        }
    }

    /// The build of `file`, which this grammar imports.
    pub(super) fn for_import(&self, file: &GrammarFile, params: Vec<Pattern>) -> Setup {
        let origin = origin(&file.resource);
        let mut chain = self.chain.clone();
        chain.push(Link::new(origin.clone(), &file.expression));
        Setup {
            resolver: self.resolver.clone(),
            origin,
            params,
            decorators: Rc::clone(&self.decorators),
            anonymous: self.anonymous.shared(),
            outer_names: HashSet::new(),
            chain,
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
            // The block is part of this grammar, not one it imports.
            chain: self.chain.clone(),
        }
    }
}
