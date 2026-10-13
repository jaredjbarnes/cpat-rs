//! Where imported grammars come from (DESIGN §8): a port the builder calls
//! synchronously, and an in-memory adapter beside it.
//!
//! The core never fetches. A host that loads grammars asynchronously (a
//! browser, a server reading files) fetches them first and answers from
//! memory, so one build never waits on the outside world.

use std::collections::HashMap;

/// A grammar's text and the name it was found under.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrammarFile {
    /// Where the text came from. The grammar's own imports are resolved
    /// relative to it: it is passed back as their `origin`.
    pub resource: String,
    /// The grammar's text.
    pub expression: String,
}

/// Loads the grammar an `import … from "resource"` names (v12
/// `resolveImportSync`).
pub trait ImportResolver {
    /// The grammar `resource` names. `origin` is the resource of the grammar
    /// doing the import (`None` for the top-level text, unless
    /// `GrammarOptions::origin_resource` names it), so relative names can be
    /// resolved against it. An `Err` is the build's error, word for word.
    fn resolve(&self, resource: &str, origin: Option<&str>) -> Result<GrammarFile, String>;
}

/// A closure is a resolver.
impl<F> ImportResolver for F
where
    F: Fn(&str, Option<&str>) -> Result<GrammarFile, String>,
{
    fn resolve(&self, resource: &str, origin: Option<&str>) -> Result<GrammarFile, String> {
        self(resource, origin)
    }
}

/// Grammars held in memory by resource name: the resolver for a host that
/// fetched every grammar ahead of time, and for tests.
#[derive(Clone, Debug, Default)]
pub struct InMemoryImports {
    files: HashMap<String, String>,
}

impl InMemoryImports {
    pub fn new() -> InMemoryImports {
        InMemoryImports::default()
    }

    /// These imports, with `resource` holding `expression`.
    pub fn with(mut self, resource: &str, expression: &str) -> InMemoryImports {
        self.insert(resource, expression);
        self
    }

    /// Adds `resource`, or replaces what it held.
    pub fn insert(&mut self, resource: &str, expression: &str) {
        self.files
            .insert(resource.to_string(), expression.to_string());
    }
}

impl ImportResolver for InMemoryImports {
    /// Names are matched exactly: `origin` plays no part.
    fn resolve(&self, resource: &str, _origin: Option<&str>) -> Result<GrammarFile, String> {
        match self.files.get(resource) {
            Some(expression) => Ok(GrammarFile {
                resource: resource.to_string(),
                expression: expression.clone(),
            }),
            None => Err(format!(
                "No grammar is held for \"{resource}\"; add it with `InMemoryImports::with`."
            )),
        }
    }
}
