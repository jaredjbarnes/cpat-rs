//! v12's introspection rules, one per pattern kind, written once for both
//! things a query can select: tokens (`getTokens`, `getTokensAfter`,
//! `getNextTokens`) and patterns (`getPatterns`, `getPatternsAfter`,
//! `getNextPatterns`). v12 writes each rule twice; the two copies differ
//! only in what a leaf contributes and in how a lookahead or cut turns
//! "what follows me" into its own answer, which is what `Select` says.

use std::rc::Rc;

use crate::pattern::Pattern;
use crate::patterns::{Choice, Repeat, Role};
use crate::tokenizable::Tokenizable;

use super::Introspection;
use super::loc::{Loc, Node};

pub(super) trait Select<'r> {
    type Item;

    /// What a leaf (literal, regex, take-until, custom) contributes.
    fn leaf(loc: &Rc<Loc<'r>>) -> Vec<Self::Item>;

    /// What `!x` and a cut contribute: they match nothing themselves, so
    /// they offer what follows them (v12 `getTokens() { return
    /// this.getNextTokens() }`).
    fn of_next(intro: &Introspection<'r>, loc: &Rc<Loc<'r>>) -> Vec<Self::Item>;

    /// The answer for a list of patterns that may come next (a sequence's
    /// `getTokensAfter` is `getPatternsAfter(…).flatMap(getTokens)`).
    fn from_patterns(intro: &Introspection<'r>, locs: Vec<Rc<Loc<'r>>>) -> Vec<Self::Item>;
}

pub(super) struct Tokens;
pub(super) struct Patterns;

impl<'r> Select<'r> for Tokens {
    type Item = String;

    fn leaf(loc: &Rc<Loc<'r>>) -> Vec<String> {
        match loc.pattern() {
            Pattern::Literal(literal) => vec![literal.token().to_string()],
            Pattern::Regex(regex) => regex.tokens().to_vec(),
            Pattern::TakeUntil(take_until) => take_until.tokens().to_vec(),
            Pattern::Custom(custom) => custom.tokens().to_vec(),
            _ => Vec::new(),
        }
    }

    fn of_next(intro: &Introspection<'r>, loc: &Rc<Loc<'r>>) -> Vec<String> {
        intro.next::<Tokens>(loc)
    }

    fn from_patterns(intro: &Introspection<'r>, locs: Vec<Rc<Loc<'r>>>) -> Vec<String> {
        locs.iter()
            .flat_map(|loc| intro.leading::<Tokens>(loc))
            .collect()
    }
}

impl<'r> Select<'r> for Patterns {
    type Item = Rc<Loc<'r>>;

    fn leaf(loc: &Rc<Loc<'r>>) -> Vec<Rc<Loc<'r>>> {
        vec![Rc::clone(loc)]
    }

    fn of_next(intro: &Introspection<'r>, loc: &Rc<Loc<'r>>) -> Vec<Rc<Loc<'r>>> {
        intro
            .next::<Patterns>(loc)
            .iter()
            .flat_map(|next| intro.leading::<Patterns>(next))
            .collect()
    }

    fn from_patterns(_: &Introspection<'r>, locs: Vec<Rc<Loc<'r>>>) -> Vec<Rc<Loc<'r>>> {
        locs
    }
}

impl<'r> Introspection<'r> {
    /// What can start `loc` (v12 `getTokens` / `getPatterns`).
    pub(super) fn leading<S: Select<'r>>(&self, loc: &Rc<Loc<'r>>) -> Vec<S::Item> {
        let pattern = match loc.node {
            Node::Operator { part, .. } => return self.sequence_leading::<S>(loc, part),
            Node::OwnContext { .. } => return self.first_child::<S>(loc),
            Node::Pattern(pattern) => pattern,
        };
        match pattern {
            Pattern::Literal(_)
            | Pattern::Regex(_)
            | Pattern::TakeUntil(_)
            | Pattern::Custom(_) => S::leaf(loc),
            Pattern::Sequence(sequence) => self.sequence_leading::<S>(loc, sequence.children()),
            Pattern::Choice(choice) => self.choice_leading::<S>(loc, choice),
            Pattern::Not(_) | Pattern::Cut(_) => S::of_next(self, loc),
            // A block starts with its open delimiter, a repeat with its
            // child (slot 0 either way).
            Pattern::Optional(_)
            | Pattern::Repeat(_)
            | Pattern::Reference(_)
            | Pattern::Context(_)
            | Pattern::Block(_)
            | Pattern::RightAssociated(_) => self.first_child::<S>(loc),
        }
    }

    /// What may follow `loc`'s child `index` (v12 `getTokensAfter` /
    /// `getPatternsAfter`). An index that is no child gets nothing.
    pub(super) fn after<S: Select<'r>>(&self, loc: &Rc<Loc<'r>>, index: usize) -> Vec<S::Item> {
        let pattern = match loc.node {
            Node::Operator { part, .. } => {
                let siblings = self.siblings_after(loc, index, part.len(), |i| &part[i]);
                return S::from_patterns(self, siblings);
            }
            Node::OwnContext { .. } => return self.next::<S>(loc),
            Node::Pattern(pattern) => pattern,
        };
        match pattern {
            Pattern::Literal(_)
            | Pattern::Regex(_)
            | Pattern::TakeUntil(_)
            | Pattern::Custom(_) => Vec::new(),
            Pattern::Sequence(sequence) => {
                let children = sequence.children();
                let siblings = self.siblings_after(loc, index, children.len(), |i| &children[i]);
                S::from_patterns(self, siblings)
            }
            Pattern::Block(block) => {
                let count = if block.content().is_some() { 3 } else { 2 };
                let child = |i| block.child_at(i).expect("i < count");
                let siblings = self.siblings_after(loc, index, count, child);
                S::from_patterns(self, siblings)
            }
            Pattern::Repeat(repeat) => {
                let patterns = self.repeat_after(loc, repeat, index);
                S::from_patterns(self, patterns)
            }
            Pattern::Choice(choice) => self.choice_after::<S>(loc, choice, index),
            // They answer for themselves whatever the child: what follows
            // them in their parent.
            Pattern::Optional(_)
            | Pattern::Not(_)
            | Pattern::Cut(_)
            | Pattern::Reference(_)
            | Pattern::Context(_)
            | Pattern::RightAssociated(_) => self.next::<S>(loc),
        }
    }

    /// What may follow `loc` in its parent (v12 `getNextTokens` /
    /// `getNextPatterns`): nothing at the root.
    pub(super) fn next<S: Select<'r>>(&self, loc: &Rc<Loc<'r>>) -> Vec<S::Item> {
        match &loc.parent {
            Some(parent) => self.after::<S>(parent, loc.step as usize),
            None => Vec::new(),
        }
    }

    fn first_child<S: Select<'r>>(&self, loc: &Rc<Loc<'r>>) -> Vec<S::Item> {
        match self.child(loc, 0) {
            Some(child) => self.leading::<S>(&child),
            None => Vec::new(),
        }
    }

    /// A sequence starts with its first child, and with the next one too
    /// while the children are optional or lookaheads. A first child that is
    /// already being expanded twice above (left recursion through
    /// references) offers nothing: expanding it again would never end.
    fn sequence_leading<S: Select<'r>>(
        &self,
        loc: &Rc<Loc<'r>>,
        children: &'r [Pattern],
    ) -> Vec<S::Item> {
        let mut items = Vec::new();
        for (index, child) in children.iter().enumerate() {
            let Some(child_loc) = self.child(loc, index) else {
                break;
            };
            if index == 0 && child_loc.is_recursive() {
                return items;
            }
            items.extend(self.leading::<S>(&child_loc));
            if !matches!(child, Pattern::Optional(_) | Pattern::Not(_)) {
                break;
            }
        }
        items
    }

    /// v12 `Sequence.getPatternsAfter` (and `Block`'s, which is the same):
    /// the next sibling, and the ones after it while they are optional;
    /// past the last child, whatever follows the parent.
    fn siblings_after(
        &self,
        loc: &Rc<Loc<'r>>,
        index: usize,
        count: usize,
        child: impl Fn(usize) -> &'r Pattern,
    ) -> Vec<Rc<Loc<'r>>> {
        // No such child. (Also keeps `index + 1` from overflowing.)
        if index >= count {
            return Vec::new();
        }
        let next = index + 1;
        if next == count {
            return self.next::<Patterns>(loc);
        }

        let mut patterns = Vec::new();
        for i in next..count {
            let Some(sibling) = self.child(loc, i) else {
                break;
            };
            patterns.push(sibling);
            if !matches!(child(i), Pattern::Optional(_)) {
                break;
            }
            if i == count - 1 {
                patterns.extend(self.next::<Patterns>(loc));
            }
        }
        patterns
    }

    /// v12 `InfiniteRepeat` / `FiniteRepeat.getPatternsAfter`. An unbounded
    /// repeat offers the divider after the child (or the child again), and
    /// what follows the repeat; the child after the divider. A bounded one
    /// offers the next slot's patterns, and what follows it after the last.
    ///
    /// v12 asks "what follows the repeat" of its `Repeat` facade, which has
    /// no parent at the root; that facade is the repeat itself here.
    fn repeat_after(&self, loc: &Rc<Loc<'r>>, repeat: &Repeat, index: usize) -> Vec<Rc<Loc<'r>>> {
        let Some(slots) = repeat.slots() else {
            let mut patterns = Vec::new();
            match (index, repeat.divider()) {
                (0, Some(_)) => patterns.extend(self.child(loc, 1)),
                (0, None) => patterns.extend(self.child(loc, 0)),
                (1, Some(_)) => return self.child(loc, 0).into_iter().collect(),
                _ => return Vec::new(),
            }
            patterns.extend(self.next::<Patterns>(loc));
            return patterns;
        };
        if index >= slots {
            Vec::new()
        } else if index + 1 == slots {
            self.next::<Patterns>(loc)
        } else {
            match self.child(loc, index + 1) {
                Some(next) => self.leading::<Patterns>(&next),
                None => Vec::new(),
            }
        }
    }

    /// v12 `Expression._selectLeading`: what may start an expression, a
    /// prefix operator or an atom. An atom already being expanded twice up
    /// this chain (it leads back into the choice through references, as in
    /// `a = b | "c"`, `b = a | "d"`) is skipped: expanding it again would
    /// never end, and it can only offer what the outer expansion does.
    fn choice_leading<S: Select<'r>>(&self, loc: &Rc<Loc<'r>>, choice: &'r Choice) -> Vec<S::Item> {
        let children = self.choice_children(loc, choice);
        let prefixes = children.iter().filter(|(role, _)| *role == Role::Prefix);
        let atoms = children
            .iter()
            .filter(|(role, child)| *role == Role::Atom && !child.is_recursive());
        prefixes
            .chain(atoms)
            .flat_map(|(_, child)| self.leading::<S>(child))
            .collect()
    }

    /// v12 `Expression._selectAfter`: after an operator still owed a right
    /// operand, what may start an expression; after a completed operand, a
    /// postfix or infix operator, or whatever follows the expression.
    fn choice_after<S: Select<'r>>(
        &self,
        loc: &Rc<Loc<'r>>,
        choice: &'r Choice,
        index: usize,
    ) -> Vec<S::Item> {
        let children = self.choice_children(loc, choice);
        let Some((role, _)) = children.get(index) else {
            return Vec::new();
        };
        match role {
            Role::Prefix | Role::Infix => self.choice_leading::<S>(loc, choice),
            Role::Atom | Role::Postfix => {
                let postfixes = children.iter().filter(|(role, _)| *role == Role::Postfix);
                let infixes = children.iter().filter(|(role, _)| *role == Role::Infix);
                let mut items: Vec<S::Item> = postfixes
                    .chain(infixes)
                    .flat_map(|(_, child)| self.leading::<S>(child))
                    .collect();
                items.extend(self.next::<S>(loc));
                items
            }
        }
    }

    /// The choice's classified children as places, with their roles.
    fn choice_children(&self, loc: &Rc<Loc<'r>>, choice: &'r Choice) -> Vec<(Role, Rc<Loc<'r>>)> {
        if loc.is_too_deep() {
            return Vec::new();
        }
        self.classified(loc, choice)
            .into_iter()
            .enumerate()
            .map_while(|(index, child)| {
                self.spend()
                    .then(|| (child.role, self.choice_child(loc, child, index)))
            })
            .collect()
    }
}
