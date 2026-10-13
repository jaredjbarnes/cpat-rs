//! The decorator registry (DESIGN §9): `@name` and `@name(json)` above a
//! rule hand the rule's pattern to the function registered as `name`, which
//! returns the pattern to keep, changed or replaced.

use std::fmt;
use std::rc::Rc;

use serde_json::Value;

use crate::pattern::Pattern;

/// A decorator: the rule's pattern and the statement's JSON argument (none
/// for `@name` and `@name()`), to the pattern the rule keeps. An `Err` is a
/// build error, printed after `[Invalid Grammar] `.
///
/// It may wrap the pattern or replace it (with a `Custom` one, say), but
/// must keep its name: other rules find it by that name.
pub type Decorator = Rc<dyn Fn(Pattern, Option<&Value>) -> Result<Pattern, String>>;

/// Decorators by name, in the order they were added, which is the order an
/// unknown decorator's message lists them in.
#[derive(Clone, Default)]
pub struct Decorators {
    entries: Vec<(String, Decorator)>,
}

impl Decorators {
    pub fn new() -> Decorators {
        Decorators::default()
    }

    /// These decorators, with `name` registered as `decorator`.
    pub fn with(
        mut self,
        name: &str,
        decorator: impl Fn(Pattern, Option<&Value>) -> Result<Pattern, String> + 'static,
    ) -> Decorators {
        self.insert(name, Rc::new(decorator));
        self
    }

    /// Registers `decorator` as `name`. A name registered again keeps its
    /// place and takes the new decorator, as a JS object's key does.
    pub fn insert(&mut self, name: &str, decorator: Decorator) {
        match self.entries.iter_mut().find(|(known, _)| known == name) {
            Some(entry) => entry.1 = decorator,
            None => self.entries.push((name.to_string(), decorator)),
        }
    }

    pub fn get(&self, name: &str) -> Option<&Decorator> {
        self.entries
            .iter()
            .find(|(known, _)| known == name)
            .map(|(_, decorator)| decorator)
    }

    /// The names, in registration order.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.entries.iter().map(|(name, _)| name.as_str())
    }

    /// These decorators and the built-in ones, as v12 merges them
    /// (`{ ...decorators, ...defaultDecorators }`): a built-in can't be
    /// replaced, and one the caller also named keeps the caller's place.
    pub(super) fn with_built_ins(&self) -> Decorators {
        let mut merged = self.clone();
        merged.insert("tokens", Rc::new(super::tokens::tokens));
        merged
    }
}

impl fmt::Debug for Decorators {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.names()).finish()
    }
}
