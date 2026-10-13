//! `import { a, b as c } from "resource" with params { … }` and `use params
//! { x, y = default }` (v12 `_resolveImportsSync`), resolved in statement
//! order before any rule is built.

use std::collections::HashSet;

use crate::pattern::Pattern;
use crate::patterns::Reference;
use crate::tree::Node;

use super::builder::{Builder, Pending};
use super::depth::MAX_IMPORT_DEPTH;
use super::error::GrammarError;
use super::imports::GrammarFile;
use super::patterns::Patterns;

impl Builder<'_, '_> {
    pub(super) fn resolve_imports(
        &mut self,
        statements: &[Node<'_, '_>],
    ) -> Result<(), GrammarError> {
        for &statement in statements {
            match statement.name() {
                "importStatement" => self.import(statement)?,
                "useParamsStatement" => self.use_params(statement),
                _ => {}
            }
        }
        Ok(())
    }

    /// One import statement: its params block is built first, then the
    /// grammar it names, and the names it asks for are taken from that.
    fn import(&mut self, statement: Node<'_, '_>) -> Result<(), GrammarError> {
        let resource = descendant(statement, "resource").value();
        let resource = &resource[1..resource.len() - 1];
        let params = self.with_params(statement)?;
        let file = self.load(resource)?;

        // v12 reports everything that goes wrong from here on as a failure
        // of this import, so an error deep in a chain of imports names each
        // grammar it passed through.
        let failed = |details| GrammarError::ImportFailed {
            resource: resource.to_string(),
            details: Box::new(details),
        };
        let setup = self.setup.for_import(&file.resource, params);
        let patterns = super::build_text(&file.expression, setup).map_err(failed)?;
        self.take_imported_names(statement, &patterns, resource)
            .map_err(failed)
    }

    /// The grammar `resource` names, from the resolver.
    fn load(&self, resource: &str) -> Result<GrammarFile, GrammarError> {
        let Some(resolver) = &self.setup.resolver else {
            return Err(GrammarError::NoImportResolver);
        };
        let file = resolver
            .resolve(resource, self.setup.origin.as_deref())
            .map_err(|message| GrammarError::ImportResolver {
                resource: resource.to_string(),
                message,
            })?;
        self.reject_import_cycle(&file.resource)?;
        Ok(file)
    }

    /// v12 recurses until the JS stack overflows when a grammar imports
    /// itself; in Rust that is a process abort, so the cycle is an error.
    fn reject_import_cycle(&self, resource: &str) -> Result<(), GrammarError> {
        let chain: Vec<&str> = self
            .setup
            .importers
            .iter()
            .chain(&self.setup.origin)
            .map(String::as_str)
            .collect();
        if let Some(start) = chain.iter().position(|&importer| importer == resource) {
            let mut cycle: Vec<String> =
                chain[start..].iter().map(|name| name.to_string()).collect();
            cycle.push(resource.to_string());
            return Err(GrammarError::ImportCycle { cycle });
        }
        // A resolver that names one grammar differently each time it is
        // imported hides the cycle; the depth still ends it.
        if chain.len() >= MAX_IMPORT_DEPTH {
            return Err(GrammarError::ImportsTooDeep {
                resource: resource.to_string(),
            });
        }
        Ok(())
    }

    /// `with params { … }`: a grammar of its own, built with what this one
    /// has imported and been given, whose rules become the import's params.
    fn with_params(&self, statement: Node<'_, '_>) -> Result<Vec<Pattern>, GrammarError> {
        let Some(block) = statement.find(|node| node.name() == "withParamStatements") else {
            return Ok(Vec::new());
        };
        let visible = self
            .imported
            .iter()
            .chain(self.params.iter())
            .map(|(_, pattern)| pattern.clone())
            .collect();
        let outer_names: HashSet<String> = self
            .setup
            .outer_names
            .iter()
            .chain(&self.rule_names)
            .cloned()
            .collect();
        let setup = self.setup.for_with_params(visible, outer_names);
        let patterns = super::build_text(block.value(), setup)?;
        Ok(patterns.into_iter().map(|(_, pattern)| pattern).collect())
    }

    /// The names the import asks for, from the grammar it loaded. An alias
    /// is a clone under the alias's name.
    fn take_imported_names(
        &mut self,
        statement: Node<'_, '_>,
        patterns: &Patterns,
        resource: &str,
    ) -> Result<(), GrammarError> {
        let names = descendant(statement, "patternNames");
        for child in names.children() {
            let (name, alias) = match child.name() {
                "importAlias" => (
                    descendant(child, "patternName").value(),
                    descendant(child, "importNameAlias").value(),
                ),
                "patternName" => (child.value(), child.value()),
                // The commas between names.
                _ => continue,
            };
            if self.imported.contains(alias) {
                return Err(GrammarError::ImportNameTaken {
                    name: alias.to_string(),
                });
            }
            let Some(pattern) = patterns.get(name) else {
                return Err(GrammarError::ImportNotFound {
                    name: name.to_string(),
                    resource: resource.to_string(),
                });
            };
            let pattern = if alias == name {
                pattern.clone()
            } else {
                pattern.clone_named(alias)
            };
            self.imported.set(alias, pattern);
        }
        Ok(())
    }

    /// `use params { x, y = default }`. A supplied param wins. A default
    /// is what an import brought in under that name, else a reference to it
    /// that must resolve by the end of the build. A param neither supplied
    /// nor defaulted is left out, so using it is an unknown name.
    fn use_params(&mut self, statement: Node<'_, '_>) {
        for param in statement.find_all(|node| node.name() == "paramNameWithDefault") {
            let name = descendant(param, "patternName").value();
            if self.params.contains(name) {
                continue;
            }
            let Some(default) = param.find(|node| node.name() == "defaultParamName") else {
                continue;
            };
            let default = default.value();
            let pattern = match self.imported.get(default) {
                Some(pattern) => pattern.clone(),
                None => {
                    self.pending.push(Pending {
                        name: default.to_string(),
                        used_in: format!("use params {{ {name} }}"),
                    });
                    Reference::new(default).into()
                }
            };
            self.imported.set(name, pattern);
        }
    }
}

/// The first node named `name` under `node`, which the cpat grammar always
/// gives it.
fn descendant<'a, 't>(node: Node<'a, 't>, name: &str) -> Node<'a, 't> {
    node.find(|child| child.name() == name)
        .unwrap_or_else(|| panic!("the cpat grammar gives `{}` a `{name}`", node.name()))
}
