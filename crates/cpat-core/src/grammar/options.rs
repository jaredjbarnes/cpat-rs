use std::fmt;
use std::rc::Rc;

use crate::pattern::Pattern;

use super::decorators::Decorators;
use super::imports::ImportResolver;

/// How to build a grammar (v12 `GrammarOptions`, synchronous).
///
/// ```
/// use cpat_core::{Grammar, GrammarOptions, InMemoryImports};
///
/// let imports = InMemoryImports::new().with("names.cpat", r#"first = "John""#);
/// let mut patterns = Grammar::parse_string(r#"
///     import { first } from "names.cpat"
///     @tokens([" "])
///     space = /\s+/
///     full = first + space + "Doe"
/// "#, GrammarOptions::default().with_resolver(imports))?;
///
/// assert!(patterns["full"].test("John Doe"));
/// assert_eq!(patterns["space"].tokens(), [" "]);
/// # Ok::<(), cpat_core::GrammarError>(())
/// ```
#[derive(Clone, Default)]
pub struct GrammarOptions {
    /// Loads the grammars `import` statements name. Without one, an import
    /// is `GrammarError::NoImportResolver`.
    pub resolver: Option<Rc<dyn ImportResolver>>,
    /// The resource the grammar's text came from, passed to the resolver as
    /// the origin of the grammar's own imports.
    pub origin_resource: Option<String>,
    /// Patterns the grammar may use by name (each under its own name): what
    /// `use params { … }` declares. A supplied param wins over a default.
    pub params: Vec<Pattern>,
    /// Decorators by name, besides the built-in `@tokens`.
    pub decorators: Decorators,
}

impl GrammarOptions {
    /// These options, loading imports with `resolver`.
    pub fn with_resolver(mut self, resolver: impl ImportResolver + 'static) -> GrammarOptions {
        self.resolver = Some(Rc::new(resolver));
        self
    }
}

impl fmt::Debug for GrammarOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GrammarOptions")
            .field("resolver", &self.resolver.as_ref().map(|_| "…"))
            .field("origin_resource", &self.origin_resource)
            .field("params", &self.params)
            .field("decorators", &self.decorators)
            .finish()
    }
}
