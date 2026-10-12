//! What a build checks once every rule is built: names that resolve to
//! nothing, aliases that never end, literals an earlier one hides.

use std::collections::HashMap;

use crate::pattern::Pattern;
use crate::tree::Node;

use super::build::string_value;
use super::builder::{Builder, alias_target};
use super::error::GrammarError;

impl Builder<'_, '_> {
    /// Every name a rule used before it was defined must be defined by the
    /// end of the build. Otherwise the grammar could never parse, so say so
    /// now instead of at first parse.
    pub(super) fn reject_unknown_names(&self) -> Result<(), GrammarError> {
        match self
            .pending
            .iter()
            .find(|pending| !self.rule_names.contains(&pending.name))
        {
            Some(unknown) => Err(GrammarError::UnknownPattern {
                name: unknown.name.clone(),
                used_in: unknown.used_in.clone(),
            }),
            None => Ok(()),
        }
    }

    /// An alias of a rule defined LATER was built as a renamed reference,
    /// and a reference never resolves to another reference, so a forward
    /// alias chain (`a = b`, `b = c`, `c = "x"`) could never parse. Point
    /// each such alias at the real pattern its chain ends in. A chain that
    /// loops back on itself never ends in a real pattern: that is an error.
    pub(super) fn resolve_aliases(&mut self) -> Result<(), GrammarError> {
        // A later alias of the same name wins, as in a JS `Map`.
        let target_of: HashMap<&str, &str> = self
            .aliases
            .iter()
            .map(|alias| (alias.name.as_str(), alias.target.as_str()))
            .collect();

        let mut resolved = Vec::new();
        for alias in &self.aliases {
            let mut chain = vec![alias.name.as_str()];
            let mut current = alias.target.as_str();
            while let Some(&next) = target_of.get(current) {
                if let Some(start) = chain.iter().position(|&name| name == current) {
                    let mut cycle: Vec<String> =
                        chain[start..].iter().map(|name| name.to_string()).collect();
                    cycle.push(current.to_string());
                    return Err(GrammarError::AliasCycle { cycle });
                }
                chain.push(current);
                current = next;
            }

            if matches!(self.rules.get(&alias.name), Some(Pattern::Reference(_))) {
                resolved.push((alias.name.clone(), current.to_string()));
            }
        }

        for (name, end) in resolved {
            let pattern = self.pattern_named(&end).clone_named(&name);
            self.rules.set(&name, pattern);
        }
        Ok(())
    }

    /// In an ordered choice, a literal can never match where an EARLIER
    /// literal alternative that is a proper prefix of it matches (`"=" |
    /// "=="` never yields `==`). Almost always the author meant
    /// longest-first, so reject it. Alternatives are seen through rule
    /// names and aliases to literal rules; anything else (a regex, a
    /// sequence) is not checked.
    pub(super) fn reject_shadowed_literals(&self) -> Result<(), GrammarError> {
        for site in &self.choices {
            let literals: Vec<Option<String>> = site
                .alternatives
                .iter()
                .map(|&alternative| self.literal_value_of(alternative))
                .collect();

            for (later, longer) in literals.iter().enumerate() {
                let Some(longer) = longer else { continue };
                let shadowing = literals[..later].iter().flatten().find(|shorter| {
                    shorter.len() < longer.len() && longer.starts_with(shorter.as_str())
                });
                if let Some(shorter) = shadowing {
                    return Err(GrammarError::ShadowedLiteral {
                        longer: longer.clone(),
                        shorter: shorter.clone(),
                        rule: site.rule.clone(),
                    });
                }
            }
        }
        Ok(())
    }

    /// The text a choice alternative matches when it is (or names) a
    /// literal.
    fn literal_value_of(&self, node: Node<'_, '_>) -> Option<String> {
        if node.name() == "literal" {
            return Some(string_value(node.value()));
        }
        match self.rules.get(alias_target(node)?)? {
            Pattern::Literal(literal) => Some(literal.token().to_string()),
            _ => None,
        }
    }
}
