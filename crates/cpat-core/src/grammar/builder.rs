//! The builder's state and its walk over a grammar's statements (v12
//! `Grammar._buildPatterns`).

use std::collections::HashSet;
use std::rc::Rc;

use crate::pattern::Pattern;
use crate::patterns::{Context, Reference};
use crate::scope;
use crate::tree::Node;

use super::anonymous::AnonymousNames;
use super::error::GrammarError;
use super::patterns::Patterns;
use super::syntax::is_skipped;

/// Everything one build learns while it walks the statements, checked
/// once the walk is done.
pub(super) struct Builder<'a, 't> {
    /// v12 `patternsByName`: each rule's pattern, by rule name.
    pub(super) rules: Patterns,
    /// Every name a rule defines, including rules not built yet.
    pub(super) rule_names: HashSet<String>,
    /// Names used before (or without) a definition.
    pub(super) pending: Vec<Pending>,
    /// Rules that only rename another (`a = b`, `a = (b)`), in order.
    pub(super) aliases: Vec<Alias>,
    /// Every `|` built, for the shadowed-literal check.
    pub(super) choices: Vec<ChoiceSite<'a, 't>>,
    pub(super) anonymous: AnonymousNames,
    /// The statement being built, for error messages.
    pub(super) current_rule: String,
}

/// A name used before (or without) a definition, and where.
pub(super) struct Pending {
    pub(super) name: String,
    pub(super) used_in: String,
}

pub(super) struct Alias {
    pub(super) name: String,
    pub(super) target: String,
}

/// A `|` and its alternatives' syntax, as written.
pub(super) struct ChoiceSite<'a, 't> {
    pub(super) rule: String,
    pub(super) alternatives: Vec<Node<'a, 't>>,
}

/// The patterns of the grammar whose tree is `root`, each wrapped in a
/// context named after its rule with every rule in scope.
pub(super) fn build(root: Node<'_, '_>) -> Result<Patterns, GrammarError> {
    let Some(statements) = root.find(|node| node.name() == "statements") else {
        return Ok(Patterns::default());
    };
    let mut builder = Builder::new(statements);
    builder.reject_unsupported(statements)?;
    builder.build_statements(statements)?;
    builder.reject_unknown_names()?;
    builder.resolve_aliases()?;
    builder.reject_shadowed_literals()?;
    Ok(record(builder.rules))
}

impl<'a, 't> Builder<'a, 't> {
    /// A builder that knows no rules yet.
    pub(super) fn empty() -> Builder<'a, 't> {
        Builder {
            rules: Patterns::default(),
            rule_names: HashSet::new(),
            pending: Vec::new(),
            aliases: Vec::new(),
            choices: Vec::new(),
            anonymous: AnonymousNames::default(),
            current_rule: String::new(),
        }
    }

    fn new(statements: Node<'a, 't>) -> Builder<'a, 't> {
        let rule_names = statements
            .children()
            .filter(|statement| statement.name() == "patternAssignment")
            .map(|statement| rule_name(&statement).to_string())
            .collect();
        Builder {
            rule_names,
            ..Builder::empty()
        }
    }

    /// v12 resolves imports and params before it builds a rule, so their
    /// mistakes come first. With no way to load an import, v12's default
    /// resolver throws; params and decorators are a later milestone, and
    /// say so rather than being ignored.
    fn reject_unsupported(&self, statements: Node<'a, 't>) -> Result<(), GrammarError> {
        for statement in statements.children() {
            match statement.name() {
                "importStatement" => return Err(GrammarError::NoImportResolver),
                "useParamsStatement" => {
                    return Err(GrammarError::Unsupported {
                        feature: "`use params` statements",
                    });
                }
                // The decoration choice gives its alternative's node.
                "decorationStatement" | "methodDecorationStatement" | "nameDecorationStatement" => {
                    return Err(GrammarError::Unsupported {
                        feature: "decorators",
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn build_statements(&mut self, statements: Node<'a, 't>) -> Result<(), GrammarError> {
        for statement in statements.children() {
            match statement.name() {
                "patternAssignment" => self.build_rule(statement)?,
                "exportPattern" => {
                    let name = statement.value();
                    self.current_rule = format!("export {name}");
                    let pattern = self.pattern_named(name);
                    self.rules.set(name, pattern);
                }
                // Comments and the newlines between statements.
                _ => {}
            }
        }
        Ok(())
    }

    /// `name = expression`.
    fn build_rule(&mut self, statement: Node<'a, 't>) -> Result<(), GrammarError> {
        let name = rule_name(&statement);
        self.current_rule = name.to_string();

        let Some(expression) = statement.children().find(|child| {
            child.name() != "patternName" && child.name() != "assign" && !is_skipped(child)
        }) else {
            return Ok(());
        };

        let alias_target = alias_target(expression);
        let mut pattern = self.build_pattern(name, expression)?;

        // An alias (`a = b`, or `a = (b)`) is the pattern it names, renamed.
        // One that names a rule defined later is still a reference here;
        // `resolve_aliases` swaps in the real pattern.
        if let Some(target) = alias_target {
            self.aliases.push(Alias {
                name: name.to_string(),
                target: target.to_string(),
            });
            if pattern.name() != name {
                pattern = pattern.clone_named(name);
            }
        }

        self.rules.set(name, pattern);
        Ok(())
    }

    /// The rule called `name` as built so far, or a reference to it when
    /// it is defined later (or nowhere: that is checked once every rule is
    /// built).
    pub(super) fn pattern_named(&mut self, name: &str) -> Pattern {
        if let Some(pattern) = self.rules.get(name) {
            return pattern.clone();
        }
        self.pending.push(Pending {
            name: name.to_string(),
            used_in: self.current_rule.clone(),
        });
        Reference::new(name).into()
    }
}

/// The name a rule defines.
fn rule_name<'t>(statement: &Node<'_, 't>) -> &'t str {
    statement
        .children()
        .find(|child| child.name() == "patternName")
        .expect("the cpat grammar names every rule")
        .value()
}

/// The name an alias right-hand side refers to (`b` in `a = b` or
/// `a = (b)`, however many parentheses), else `None`.
pub(super) fn alias_target<'t>(node: Node<'_, 't>) -> Option<&'t str> {
    let mut current = node;
    while current.name() == "patternGroupExpr" {
        current = group_inner(current)?;
    }
    (current.name() == "patternIdentifier").then(|| current.value())
}

/// What a `( … )` group holds.
pub(super) fn group_inner<'a, 't>(group: Node<'a, 't>) -> Option<Node<'a, 't>> {
    group
        .children()
        .find(|child| !is_skipped(child) && child.name() != "(" && child.name() != ")")
}

/// v12 `_buildPatternRecord`: every rule in a context named after it, with
/// every rule in scope. The contexts share one table of the rules (the
/// table holds each rule's own definition too, which its context never
/// reaches: a context answers its own name first).
fn record(rules: Patterns) -> Patterns {
    let definitions = Rc::new(scope::definitions(
        rules.iter().map(|(_, pattern)| pattern.clone()).collect(),
    ));
    rules.map(|name, pattern| Context::sharing(name, pattern, Rc::clone(&definitions)).into())
}
