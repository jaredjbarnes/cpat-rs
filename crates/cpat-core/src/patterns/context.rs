use std::collections::HashMap;
use std::rc::Rc;

use crate::cursor::Cursor;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
use crate::scope::{Definitions, Scope, ScopeFrame};
use crate::tree::NodeId;

/// A pattern parsed with extra definitions in scope: references inside it
/// (and inside whatever they resolve to) can name `patterns`, and the
/// context itself by its name.
///
/// It adds no node of its own: the result is its child's.
#[derive(Clone, Debug)]
pub struct Context {
    pub(crate) meta: Meta,
    pub(crate) child: Box<Pattern>,
    scope: Rc<Scope>,
}

impl Context {
    /// `patterns` are kept as they are: they are definitions to look up, not
    /// children (v12 doesn't clone them either).
    pub fn new(name: &str, child: Pattern, patterns: Vec<Pattern>) -> Context {
        let meta = Meta::new(name);
        let child = child.adopted();
        let scope = Scope::for_context(meta.name, child.clone(), patterns);
        Context {
            meta,
            child: Box::new(child),
            scope: Rc::new(scope),
        }
    }

    /// A context whose definitions are a table it shares with others: a
    /// grammar wraps each rule in a context over all the rules (v12
    /// `_buildPatternRecord`), and one table serves them all instead of a
    /// copy of every other rule per rule. The table may hold the child's own
    /// definition too: the context answers its own name first, so a lookup
    /// never reaches it.
    pub(crate) fn sharing(name: &str, child: Pattern, definitions: Rc<Definitions>) -> Context {
        let meta = Meta::new(name);
        let child = child.adopted();
        let scope = Scope::sharing(meta.name, child.clone(), definitions);
        Context {
            meta,
            child: Box::new(child),
            scope: Rc::new(scope),
        }
    }

    /// The context a lookup of its own name returns: same identity and
    /// scope, a fresh child.
    pub(crate) fn from_scope(meta: Meta, child: Pattern, scope: Rc<Scope>) -> Context {
        Context {
            meta,
            child: Box::new(child),
            scope,
        }
    }

    pub fn child(&self) -> &Pattern {
        &self.child
    }

    /// The patterns this context puts in scope, by name. A new map each
    /// call: it lists the context's definitions without exposing its state.
    pub fn patterns_within_context(&self) -> HashMap<&'static str, &Pattern> {
        self.scope
            .patterns()
            .map(|(name, pattern)| (name.as_str(), pattern))
            .collect()
    }

    /// The definitions this context puts in scope, borrowed (introspection
    /// looks names up without parsing).
    pub(crate) fn scope(&self) -> &Scope {
        &self.scope
    }

    pub(crate) fn frame(&self) -> ScopeFrame {
        ScopeFrame::new(Rc::clone(&self.scope), self.meta)
    }

    pub(crate) fn parse(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        cursor.push_scope(self.frame());
        let node = self.child.parse(cursor);
        cursor.pop_scope();
        node
    }
}
