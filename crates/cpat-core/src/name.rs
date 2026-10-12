use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;

/// An interned pattern name.
///
/// Nodes carry a `Name` (a `u32`) instead of a string, so building a tree never
/// copies names and comparing two names is an integer compare. `as_str` hands
/// back the text for visitors that match on names.
// `Ord` orders by interning order: arbitrary, but stable within a thread,
// which is all a sorted key needs.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(u32);

#[derive(Default)]
struct Interner {
    ids: HashMap<&'static str, u32>,
    strs: Vec<&'static str>,
}

thread_local! {
    // Names are leaked on purpose: there is one per distinct pattern name, a
    // number bounded by the grammars a program builds, and leaking lets
    // `as_str` return `&'static str` without tying nodes to the interner.
    // Parsing is single-threaded by design (no fan-out), so a thread-local
    // table is the whole story.
    static NAMES: RefCell<Interner> = RefCell::new(Interner::default());
}

impl Name {
    pub fn new(name: &str) -> Name {
        NAMES.with(|names| {
            let mut names = names.borrow_mut();

            if let Some(&id) = names.ids.get(name) {
                return Name(id);
            }

            let leaked: &'static str = Box::leak(name.to_owned().into_boxed_str());
            let id = names.strs.len() as u32;
            names.strs.push(leaked);
            names.ids.insert(leaked, id);
            Name(id)
        })
    }

    pub fn as_str(self) -> &'static str {
        NAMES.with(|names| names.borrow().strs[self.0 as usize])
    }
}

impl From<&str> for Name {
    fn from(name: &str) -> Name {
        Name::new(name)
    }
}

impl PartialEq<&str> for Name {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_str())
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
