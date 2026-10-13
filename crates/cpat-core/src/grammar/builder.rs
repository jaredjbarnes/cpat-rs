//! The builder's state and its walk over a grammar's statements (v12
//! `Grammar._buildPatterns`).

use std::collections::HashSet;
use std::rc::Rc;

use crate::pattern::Pattern;
use crate::patterns::{Context, Reference};
use crate::scope;
use crate::tree::Node;

use super::error::GrammarError;
use super::patterns::Patterns;
use super::setup::Setup;
use super::syntax::is_skipped;

/// Everything one build learns while it walks the statements, checked
/// once the walk is done.
pub(super) struct Builder<'a, 't> {
    /// What the build was given.
    pub(super) setup: Setup,
    /// v12 `patternsByName`: each rule's pattern, by rule name.
    pub(super) rules: Patterns,
    /// v12 `importedPatternsByName`: what imports and `use params` defaults
    /// brought in, by the name the grammar uses.
    pub(super) imported: Patterns,
    /// v12 `paramsByName`: the params supplied, by their patterns' names.
    pub(super) params: Patterns,
    /// Every name a rule defines, including rules not built yet.
    pub(super) rule_names: HashSet<String>,
    /// Names used before (or without) a definition.
    pub(super) pending: Vec<Pending>,
    /// Rules that only rename another of this grammar's rules (`a = b`,
    /// `a = (b)`), in order.
    pub(super) aliases: Vec<Alias>,
    /// Every `|` built, for the shadowed-literal check.
    pub(super) choices: Vec<ChoiceSite<'a, 't>>,
    /// The statement being built, for error messages.
    pub(super) current_rule: String,
    /// The grammar's text, for the line and column of an error.
    pub(super) text: &'t str,
    /// How many patterns are being built inside one another right now.
    pub(super) depth: usize,
}

/// A name used before (or without) a definition, and where.
pub(super) struct Pending {
    pub(super) name: String,
    pub(super) used_in: String,
}

pub(super) struct Alias {
    pub(super) name: String,
    pub(super) target: String,
    /// Where its statement is, so its decorators can be found once it is
    /// resolved.
    pub(super) statement: usize,
}

/// A `|` and its alternatives' syntax, as written.
pub(super) struct ChoiceSite<'a, 't> {
    pub(super) rule: String,
    pub(super) alternatives: Vec<Node<'a, 't>>,
}

/// The patterns of the grammar whose tree is `root`, each wrapped in a
/// context named after its rule with every rule in scope.
pub(super) fn build<'t>(
    text: &'t str,
    root: Node<'_, 't>,
    setup: Setup,
) -> Result<Patterns, GrammarError> {
    let Some(statements) = root.find(|node| node.name() == "statements") else {
        return Ok(Patterns::default());
    };
    let statements: Vec<Node<'_, 't>> = statements.children().collect();
    let mut builder = Builder::new(text, &statements, setup);
    builder.resolve_imports(&statements)?;
    builder.build_statements(&statements)?;
    builder.reject_unknown_names()?;
    builder.resolve_aliases(&statements)?;
    builder.reject_shadowed_literals()?;
    Ok(record(builder.rules))
}

impl<'a, 't> Builder<'a, 't> {
    /// A builder that knows no rules yet.
    pub(super) fn empty(text: &'t str, setup: Setup) -> Builder<'a, 't> {
        let params = setup
            .params
            .iter()
            .fold(Patterns::default(), |mut params, param| {
                params.set(param.name(), param.clone());
                params
            });
        Builder {
            setup,
            rules: Patterns::default(),
            imported: Patterns::default(),
            params,
            rule_names: HashSet::new(),
            pending: Vec::new(),
            aliases: Vec::new(),
            choices: Vec::new(),
            current_rule: String::new(),
            text,
            depth: 0,
        }
    }

    fn new(text: &'t str, statements: &[Node<'a, 't>], setup: Setup) -> Builder<'a, 't> {
        let rule_names = statements
            .iter()
            .filter(|statement| statement.name() == "patternAssignment")
            .map(|statement| rule_name(statement).to_string())
            .collect();
        Builder {
            rule_names,
            ..Builder::empty(text, setup)
        }
    }

    fn build_statements(&mut self, statements: &[Node<'a, 't>]) -> Result<(), GrammarError> {
        for (index, &statement) in statements.iter().enumerate() {
            match statement.name() {
                "patternAssignment" => self.build_rule(statements, index)?,
                "exportPattern" => {
                    let name = statement.value();
                    self.current_rule = format!("export {name}");
                    let pattern = self.pattern_named(name);
                    self.rules.set(name, pattern);
                }
                // Imports and params were resolved before any rule;
                // decorators are read from the rule they decorate.
                // Comments and the newlines between statements build nothing.
                _ => {}
            }
        }
        Ok(())
    }

    /// `name = expression`, the statement at `index`.
    fn build_rule(
        &mut self,
        statements: &[Node<'a, 't>],
        index: usize,
    ) -> Result<(), GrammarError> {
        let statement = statements[index];
        let name = rule_name(&statement);
        self.current_rule = name.to_string();

        let Some(expression) = statement.children().find(|child| {
            child.name() != "patternName" && child.name() != "assign" && !is_skipped(child)
        }) else {
            return Ok(());
        };

        let alias_target = alias_target(expression);
        // Whether the alias names one of THIS grammar's rules, as opposed to
        // an import or param of the same name (`space = space` passes a param
        // through): only those can form an alias cycle.
        let local_alias = alias_target.is_some_and(|target| self.is_local_name(target));
        let mut pattern = self.build_pattern(name, expression)?;

        // An alias (`a = b`, or `a = (b)`) is the pattern it names, renamed.
        // One that names a rule defined later is still a reference here;
        // `resolve_aliases` swaps in the real pattern.
        if let Some(target) = alias_target {
            if local_alias {
                self.aliases.push(Alias {
                    name: name.to_string(),
                    target: target.to_string(),
                    statement: index,
                });
            }
            if pattern.name() != name {
                pattern = pattern.clone_named(name);
            }
        }

        // A forward alias is still a reference: its decorators apply once
        // `resolve_aliases` swaps in the real pattern, so they apply once.
        if !(local_alias && matches!(pattern, Pattern::Reference(_))) {
            pattern = self.decorate(statements, index, pattern)?;
        }
        self.rules.set(name, pattern);
        Ok(())
    }

    /// The pattern called `name` as built so far: a rule, else an import,
    /// else a param. Otherwise a reference to it, when it is defined later
    /// (or nowhere: that is checked once every rule is built).
    pub(super) fn pattern_named(&mut self, name: &str) -> Pattern {
        if let Some(pattern) = self.known(name) {
            return pattern.clone();
        }
        self.pending.push(Pending {
            name: name.to_string(),
            used_in: self.current_rule.clone(),
        });
        Reference::new(name).into()
    }

    /// v12 `_getPattern`'s lookup, without the fallback.
    pub(super) fn known(&self, name: &str) -> Option<&Pattern> {
        self.rules
            .get(name)
            .or_else(|| self.imported.get(name))
            .or_else(|| self.params.get(name))
    }

    /// Whether `name`, looked up now, is (or will be) a rule of this grammar.
    fn is_local_name(&self, name: &str) -> bool {
        self.rules.contains(name) || !(self.imported.contains(name) || self.params.contains(name))
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
