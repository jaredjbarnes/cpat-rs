use std::collections::HashMap;
use std::ops::{Index, IndexMut};

use crate::pattern::Pattern;

/// A built grammar's patterns by rule name, in the order the rules were
/// first defined (v12's record, whose keys keep insertion order).
///
/// Each pattern is a `Context` named after its rule, wrapping the rule's
/// pattern with every rule in scope, so a pattern taken out of the record
/// still resolves the names it uses.
#[derive(Clone, Debug, Default)]
pub struct Patterns {
    names: Vec<String>,
    patterns: HashMap<String, Pattern>,
}

impl Patterns {
    pub fn get(&self, name: &str) -> Option<&Pattern> {
        self.patterns.get(name)
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut Pattern> {
        self.patterns.get_mut(name)
    }

    /// Takes the pattern out of the record.
    pub fn remove(&mut self, name: &str) -> Option<Pattern> {
        let pattern = self.patterns.remove(name)?;
        self.names.retain(|kept| kept != name);
        Some(pattern)
    }

    pub fn contains(&self, name: &str) -> bool {
        self.patterns.contains_key(name)
    }

    /// The rule names, in definition order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.names.iter().map(String::as_str)
    }

    /// `(name, pattern)` in definition order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &Pattern)> {
        self.names
            .iter()
            .map(|name| (name.as_str(), &self.patterns[name]))
    }

    pub fn len(&self) -> usize {
        self.names.len()
    }

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }

    /// Defines or redefines `name`. A redefinition keeps the name's place,
    /// as a JS `Map.set` does.
    pub(crate) fn set(&mut self, name: &str, pattern: Pattern) {
        if self.patterns.insert(name.to_string(), pattern).is_none() {
            self.names.push(name.to_string());
        }
    }

    /// `f` applied to every pattern, keeping names and order.
    pub(crate) fn map(self, mut f: impl FnMut(&str, Pattern) -> Pattern) -> Patterns {
        let Patterns {
            names,
            mut patterns,
        } = self;
        let patterns = names
            .iter()
            .map(|name| {
                let pattern = patterns.remove(name).expect("every name has a pattern");
                (name.clone(), f(name, pattern))
            })
            .collect();
        Patterns { names, patterns }
    }
}

impl Index<&str> for Patterns {
    type Output = Pattern;

    /// Panics when no rule is called `name`, as indexing a map does.
    fn index(&self, name: &str) -> &Pattern {
        self.get(name)
            .unwrap_or_else(|| panic!("no rule is called `{name}`"))
    }
}

impl IndexMut<&str> for Patterns {
    fn index_mut(&mut self, name: &str) -> &mut Pattern {
        self.get_mut(name)
            .unwrap_or_else(|| panic!("no rule is called `{name}`"))
    }
}

impl IntoIterator for Patterns {
    type Item = (String, Pattern);
    type IntoIter = std::vec::IntoIter<(String, Pattern)>;

    /// `(name, pattern)` in definition order.
    fn into_iter(mut self) -> Self::IntoIter {
        let names = std::mem::take(&mut self.names);
        names
            .into_iter()
            .map(|name| {
                let pattern = self
                    .patterns
                    .remove(&name)
                    .expect("every name has a pattern");
                (name, pattern)
            })
            .collect::<Vec<_>>()
            .into_iter()
    }
}
