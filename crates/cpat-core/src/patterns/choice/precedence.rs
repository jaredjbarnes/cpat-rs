//! Assembles a Pratt parse's operands and operators into a tree (v12
//! `PrecedenceTree`), then writes it into the arena at commit.
//!
//! v12 builds this with its mutable `Node`s and three placeholder nodes
//! marking where the next operand goes. The arena is append-only and its
//! nodes are written children first, so the shape is assembled here in a
//! small scratch tree of slots with the same operations (`append`,
//! `remove`, `replace_with`, `root`), step for step as v12 does them, and
//! only the finished tree is written to the arena.

use crate::cursor::Cursor;
use crate::name::Name;
use crate::tree::{NodeId, NodeKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Association {
    Left,
    Right,
}

/// Precedence and association by operator name. Precedence is the
/// alternative's declaration index: lower binds tighter. A later
/// alternative with the same name replaces an earlier one's entry, as v12's
/// record assignment does.
#[derive(Clone, Debug, Default)]
pub(super) struct Table {
    entries: Vec<(Name, usize, Option<Association>)>,
}

impl Table {
    pub(super) fn set_precedence(&mut self, name: Name, precedence: usize) {
        self.entry(name).1 = precedence;
    }

    pub(super) fn set_association(&mut self, name: Name, association: Association) {
        self.entry(name).2 = Some(association);
    }

    fn entry(&mut self, name: Name) -> &mut (Name, usize, Option<Association>) {
        let at = match self.entries.iter().position(|e| e.0 == name) {
            Some(at) => at,
            None => {
                self.entries.push((name, 0, None));
                self.entries.len() - 1
            }
        };
        &mut self.entries[at]
    }

    fn precedence(&self, name: Name) -> Option<usize> {
        self.entries.iter().find(|e| e.0 == name).map(|e| e.1)
    }

    /// v12 `_getPrecedence`: an unknown name counts as 0.
    fn precedence_or_zero(&self, name: Name) -> usize {
        self.precedence(name).unwrap_or(0)
    }

    fn is_right(&self, name: Name) -> bool {
        self.entries
            .iter()
            .any(|e| e.0 == name && e.2 == Some(Association::Right))
    }
}

/// A node of the scratch tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Slot(u32);

#[derive(Clone, Copy, Debug)]
enum Kind {
    /// An operator applied to its operands: an expression node to write.
    Operator(Name),
    /// Where the next operand goes.
    Placeholder,
    /// A node already in the arena: an operand, or a part of an operator.
    Parsed(NodeId),
}

#[derive(Clone, Debug)]
struct Item {
    kind: Kind,
    parent: Option<Slot>,
    children: Vec<Slot>,
}

/// How to undo the last binary operator when no operand follows it
/// (v12's `_revertBinary` closures, as data).
#[derive(Clone, Copy, Debug)]
enum Revert {
    Nothing,
    First { atom: Slot },
    RightEqual { node: Slot, atom: Slot, last: Slot },
    LeftEqual { node: Slot, last: Slot },
    Climb { node: Slot, root: Slot },
    Tighter { node: Slot, atom: Slot, last: Slot },
}

#[derive(Clone, Debug)]
pub(super) struct PrecedenceTree {
    table: Table,
    items: Vec<Item>,
    prefix_placeholder: Slot,
    postfix_placeholder: Slot,
    binary_placeholder: Slot,
    prefix: Option<Slot>,
    postfix: Option<Slot>,
    binary: Option<Slot>,
    atom: Option<Slot>,
    revert: Revert,
}

impl PrecedenceTree {
    pub(super) fn new(table: Table) -> PrecedenceTree {
        let mut tree = PrecedenceTree {
            table,
            items: Vec::new(),
            prefix_placeholder: Slot(0),
            postfix_placeholder: Slot(1),
            binary_placeholder: Slot(2),
            prefix: None,
            postfix: None,
            binary: None,
            atom: None,
            revert: Revert::Nothing,
        };
        tree.reset();
        tree
    }

    /// Forgets everything added since the last commit.
    pub(super) fn reset(&mut self) {
        self.items.clear();
        self.prefix_placeholder = self.add(Kind::Placeholder, Vec::new());
        self.postfix_placeholder = self.add(Kind::Placeholder, Vec::new());
        self.binary_placeholder = self.add(Kind::Placeholder, Vec::new());
        self.prefix = None;
        self.postfix = None;
        self.binary = None;
        self.atom = None;
        self.revert = Revert::Nothing;
    }

    pub(super) fn has_atom(&self) -> bool {
        self.atom.is_some()
    }

    pub(super) fn add_atom(&mut self, node: NodeId) {
        self.atom = Some(self.add(Kind::Parsed(node), Vec::new()));
    }

    /// A prefix operator `name` whose sequence matched `parts`. Each prefix
    /// nests inside the one before it.
    pub(super) fn add_prefix(&mut self, name: Name, parts: &[NodeId]) {
        let parts = self.parsed(parts);
        let node = self.add(Kind::Operator(name), parts);
        if self.prefix.is_some() {
            self.replace_with(self.prefix_placeholder, node);
        }
        self.append(node, self.prefix_placeholder);
        self.prefix = Some(node);
    }

    /// A postfix operator `name` whose sequence matched `parts`. Each
    /// postfix wraps the one before it.
    pub(super) fn add_postfix(&mut self, name: Name, parts: &[NodeId]) {
        let operand = self.postfix.unwrap_or(self.postfix_placeholder);
        let mut children = vec![operand];
        children.extend(self.parsed(parts));
        self.postfix = Some(self.add(Kind::Operator(name), children));
    }

    /// An infix operator `name` whose sequence matched `parts`, after the
    /// operand just added. Called only when there is one.
    pub(super) fn add_binary(&mut self, name: Name, parts: &[NodeId]) {
        let last = self.binary;
        let last_precedence = last.map_or(0, |last| self.precedence_of(last));
        let precedence = self.table.precedence_or_zero(name);
        let right = self.table.is_right(name);
        let atom = self
            .compile_atom()
            .expect("an infix operator is only tried after an operand");
        let parts = self.parsed(parts);
        let placeholder = self.binary_placeholder;

        self.remove(placeholder);

        let Some(last) = last else {
            let node = self.operator(name, Some(atom), parts);
            self.binary = Some(node);
            self.revert = Revert::First { atom };
            return;
        };

        let node = if precedence == last_precedence && right {
            // Right-associated at the same level: nests to the right.
            let node = self.operator(name, Some(atom), parts);
            self.append(last, node);
            self.revert = Revert::RightEqual { node, atom, last };
            node
        } else if precedence == last_precedence {
            // Same level, left-associated: the last operator becomes this
            // one's left operand.
            let node = self.add(Kind::Operator(name), Vec::new());
            self.replace_with(last, node);
            self.append(last, atom);
            self.append(node, last);
            self.append_operator_rest(node, parts);
            self.revert = Revert::LeftEqual { node, last };
            node
        } else if precedence > last_precedence {
            // Binds looser: climb to the loosest ancestor it still binds
            // tighter than (or as tight as), and take that subtree as the
            // left operand.
            let mut root = last;
            let mut ancestor = self.items[last.0 as usize].parent;
            while let Some(at) = ancestor {
                let at_name = self.operator_name(at);
                let at_precedence = at_name.and_then(|n| self.table.precedence(n));
                if at_precedence.is_some_and(|p| p > precedence) {
                    break;
                }
                // A right-associated ancestor of EQUAL precedence keeps the
                // incoming operator nested to its right: `a as b + c as d`
                // (`+` tighter than a right-associated `as`) is
                // `a as ((b + c) as d)`, not `(a as (b + c)) as d`.
                if at_precedence == Some(precedence)
                    && at_name.is_some_and(|n| self.table.is_right(n))
                {
                    break;
                }
                root = at;
                ancestor = self.items[at.0 as usize].parent;
            }

            self.append(last, atom);
            let node = self.add(Kind::Operator(name), Vec::new());
            self.replace_with(root, node);
            self.append(node, root);
            self.append_operator_rest(node, parts);
            self.revert = Revert::Climb { node, root };
            node
        } else {
            // Binds tighter: takes the operand just added and hangs off the
            // last operator as its right operand.
            let node = self.operator(name, Some(atom), parts);
            self.append(last, node);
            self.revert = Revert::Tighter { node, atom, last };
            node
        };
        self.binary = Some(node);
    }

    /// Writes the assembled tree into the arena and returns its root, or
    /// `None` when no operand was ever completed. A trailing infix operator
    /// with no right operand is dropped. An operand on its own (no
    /// operators) is returned as it is, unwrapped.
    pub(super) fn commit(&mut self, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let atom = self.compile_atom();
        let root = match (self.binary, atom) {
            (None, atom) => atom,
            (Some(_), None) => {
                self.undo_binary();
                self.binary.map(|binary| self.root(binary))
            }
            (Some(binary), Some(atom)) => {
                self.replace_with(self.binary_placeholder, atom);
                Some(self.root(binary))
            }
        };
        let node = root.map(|root| self.write(root, cursor));
        self.reset();
        node
    }

    // ─── Assembly ───────────────────────────────────────────────────────

    /// The operand just completed, with the prefix and postfix operators
    /// around it (v12 `_compileAtomNode`). Clears them.
    fn compile_atom(&mut self) -> Option<Slot> {
        let mut node = self.atom;

        match (self.prefix, self.postfix, self.atom) {
            (Some(prefix), Some(postfix), Some(atom)) => {
                // Precedence is the declaration index, so the operator with
                // the LOWER number binds tighter and goes inside.
                let prefix_first = self.precedence_of(prefix) <= self.precedence_of(postfix);
                let (first, second, first_placeholder, second_placeholder) = if prefix_first {
                    (
                        prefix,
                        postfix,
                        self.prefix_placeholder,
                        self.postfix_placeholder,
                    )
                } else {
                    (
                        postfix,
                        prefix,
                        self.postfix_placeholder,
                        self.prefix_placeholder,
                    )
                };
                let inner = self.root(first);
                self.replace_with(first_placeholder, atom);
                self.replace_with(second_placeholder, inner);
                node = Some(second);
            }
            (prefix, postfix, atom) => {
                if let (Some(prefix), Some(atom)) = (prefix, atom) {
                    node = Some(self.root(prefix));
                    self.replace_with(self.prefix_placeholder, atom);
                }
                if let (Some(postfix), Some(operand)) = (postfix, node) {
                    self.replace_with(self.postfix_placeholder, operand);
                    node = Some(postfix);
                }
            }
        }

        self.prefix = None;
        self.atom = None;
        self.postfix = None;
        node.map(|node| self.root(node))
    }

    fn undo_binary(&mut self) {
        match self.revert {
            Revert::Nothing => {}
            Revert::First { atom } => {
                self.remove(atom);
                self.binary = Some(atom);
            }
            Revert::RightEqual { node, atom, last } => {
                self.replace_with(node, atom);
                self.binary = Some(last);
            }
            Revert::LeftEqual { node, last } => {
                self.remove(last);
                self.replace_with(node, last);
                self.binary = Some(last);
            }
            Revert::Climb { node, root } => {
                self.remove(root);
                self.replace_with(node, root);
                self.binary = Some(root);
            }
            Revert::Tighter { node, atom, last } => {
                self.remove(atom);
                self.replace_with(node, atom);
                self.binary = Some(last);
            }
        }
    }

    /// `[left, parts…, placeholder]`: an infix operator node waiting for its
    /// right operand.
    fn operator(&mut self, name: Name, left: Option<Slot>, parts: Vec<Slot>) -> Slot {
        let node = self.add(Kind::Operator(name), left.into_iter().collect());
        self.append_operator_rest(node, parts);
        node
    }

    fn append_operator_rest(&mut self, node: Slot, parts: Vec<Slot>) {
        for part in parts {
            self.append(node, part);
        }
        self.append(node, self.binary_placeholder);
    }

    fn precedence_of(&self, slot: Slot) -> usize {
        self.operator_name(slot)
            .map_or(0, |name| self.table.precedence_or_zero(name))
    }

    fn operator_name(&self, slot: Slot) -> Option<Name> {
        match self.items[slot.0 as usize].kind {
            Kind::Operator(name) => Some(name),
            Kind::Placeholder | Kind::Parsed(_) => None,
        }
    }

    // ─── The scratch tree (v12 `Node`'s mutations) ──────────────────────

    fn add(&mut self, kind: Kind, children: Vec<Slot>) -> Slot {
        let slot = Slot(self.items.len() as u32);
        for &child in &children {
            self.items[child.0 as usize].parent = Some(slot);
        }
        self.items.push(Item {
            kind,
            parent: None,
            children,
        });
        slot
    }

    fn parsed(&mut self, nodes: &[NodeId]) -> Vec<Slot> {
        nodes
            .iter()
            .map(|&node| self.add(Kind::Parsed(node), Vec::new()))
            .collect()
    }

    fn append(&mut self, parent: Slot, child: Slot) {
        self.items[child.0 as usize].parent = Some(parent);
        self.items[parent.0 as usize].children.push(child);
    }

    fn remove(&mut self, child: Slot) {
        if let Some(parent) = self.items[child.0 as usize].parent.take() {
            self.items[parent.0 as usize]
                .children
                .retain(|&c| c != child);
        }
    }

    fn replace_with(&mut self, old: Slot, new: Slot) {
        let Some(parent) = self.items[old.0 as usize].parent else {
            return;
        };
        let children = &mut self.items[parent.0 as usize].children;
        if let Some(at) = children.iter().position(|&c| c == old) {
            children[at] = new;
            self.items[new.0 as usize].parent = Some(parent);
            self.items[old.0 as usize].parent = None;
        }
    }

    fn root(&self, mut slot: Slot) -> Slot {
        while let Some(parent) = self.items[slot.0 as usize].parent {
            slot = parent;
        }
        slot
    }

    /// Writes `root`'s subtree into the arena, children first, and returns
    /// the node written for `root`. An operator node spans its first
    /// child's start to its last child's end.
    ///
    /// The walk keeps its own stack instead of recursing. The Pratt loop is
    /// iterative, so the depth limit never sees an operator chain, and
    /// `a+a+…+a` or `--…-a` builds a tree as deep as the input is long: a
    /// recursive write overflowed the native stack (a process abort) on
    /// long inputs that v12 parses or rejects with a catchable error.
    fn write(&self, root: Slot, cursor: &mut Cursor<'_>) -> NodeId {
        // Each frame is a slot and how many of its children are written.
        // Written nodes wait on `written` until their parent takes them.
        let mut pending = vec![(root, 0usize)];
        let mut written: Vec<NodeId> = Vec::new();

        while let Some((slot, done)) = pending.pop() {
            let item = &self.items[slot.0 as usize];
            match item.kind {
                Kind::Parsed(node) => written.push(node),
                Kind::Operator(_) if done < item.children.len() => {
                    pending.push((slot, done + 1));
                    pending.push((item.children[done], 0));
                }
                Kind::Operator(name) => {
                    let children = written.split_off(written.len() - item.children.len());
                    let start = cursor.node_start(children[0]);
                    let end = cursor.node_end(children[children.len() - 1]);
                    written.push(cursor.branch(NodeKind::Expression, name, start, end, &children));
                }
                // Every placeholder is filled or cut away before a commit
                // writes anything: a completed operand fills the prefix,
                // postfix and binary placeholders it is compiled into, and a
                // missing one drops the operators waiting for it.
                Kind::Placeholder => {
                    unreachable!("a placeholder is never part of a committed tree")
                }
            }
        }

        written
            .pop()
            .expect("writing a subtree leaves exactly its root")
    }
}

/// v12's `PrecedenceTree.test.ts`, call for call. Each operand or operator
/// part is a leaf over its token's first unused occurrence in the text, so
/// expected shapes show ranges too, in reading order even where v12 adds a
/// postfix before its operand.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::tree::Node;

    struct Fixture {
        cursor: Cursor<'static>,
        used: Vec<bool>,
        tree: PrecedenceTree,
    }

    impl Fixture {
        fn new(text: &'static str, precedence: &[(&str, usize)], right: &[&str]) -> Fixture {
            let mut table = Table::default();
            for &(name, index) in precedence {
                table.set_precedence(Name::new(name), index);
            }
            for &name in right {
                table.set_association(Name::new(name), Association::Right);
            }
            Fixture {
                cursor: Cursor::new(text),
                used: vec![false; text.len()],
                tree: PrecedenceTree::new(table),
            }
        }

        /// A leaf named `token` over its first unused occurrence.
        fn leaf(&mut self, token: &str) -> NodeId {
            let text = self.cursor.text();
            let start = (0..=text.len() - token.len())
                .find(|&at| {
                    text[at..].starts_with(token)
                        && !self.used[at..at + token.len()].contains(&true)
                })
                .expect("the token is in the text");
            let end = start + token.len();
            self.used[start..end].fill(true);
            self.cursor
                .leaf(NodeKind::Literal, Name::new(token), start, end)
        }

        fn atom(&mut self, token: &str) {
            let node = self.leaf(token);
            self.tree.add_atom(node);
        }

        fn prefix(&mut self, name: &str, token: &str) {
            let node = self.leaf(token);
            self.tree.add_prefix(Name::new(name), &[node]);
        }

        fn postfix(&mut self, name: &str, token: &str) {
            let node = self.leaf(token);
            self.tree.add_postfix(Name::new(name), &[node]);
        }

        fn binary(&mut self, name: &str, token: &str) {
            let node = self.leaf(token);
            self.tree.add_binary(Name::new(name), &[node]);
        }

        fn commit(mut self) -> (String, String) {
            let root = self.tree.commit(&mut self.cursor).expect("a tree");
            let tree = self.cursor.into_tree(root);
            (shape(tree.root()), tree.root().value().to_string())
        }
    }

    /// `name@start..end[children]`, with `:e` marking expression nodes.
    fn shape(node: Node<'_, '_>) -> String {
        let children: Vec<String> = node.children().map(shape).collect();
        let children = if children.is_empty() {
            String::new()
        } else {
            format!("[{}]", children.join(" "))
        };
        let kind = if node.kind() == NodeKind::Expression {
            ":e"
        } else {
            ""
        };
        format!(
            "{}{kind}@{}..{}{children}",
            node.name(),
            node.start(),
            node.end()
        )
    }

    const LEVELS: &[(&str, usize)] = &[("mul", 0), ("add", 1), ("bool", 2)];

    #[test]
    fn add_binary() {
        let mut t = Fixture::new("a+b*c||d+e", LEVELS, &[]);
        t.atom("a");
        t.binary("add", "+");
        t.atom("b");
        t.binary("mul", "*");
        t.atom("c");
        t.binary("bool", "||");
        t.atom("d");
        t.binary("add", "+");
        t.atom("e");

        assert_eq!(
            t.commit(),
            (
                "bool:e@0..10[add:e@0..5[a@0..1 +@1..2 mul:e@2..5[b@2..3 *@3..4 c@4..5]] \
                 ||@5..7 add:e@7..10[d@7..8 +@8..9 e@9..10]]"
                    .to_string(),
                "a+b*c||d+e".to_string()
            )
        );
    }

    #[test]
    fn add_prefix() {
        let mut t = Fixture::new("!+++a", &[], &[]);
        t.prefix("negate", "!");
        t.prefix("increment", "++");
        t.prefix("plus", "+");
        t.atom("a");

        assert_eq!(
            t.commit().0,
            "negate:e@0..5[!@0..1 increment:e@1..5[++@1..3 plus:e@3..5[+@3..4 a@4..5]]]"
        );
    }

    #[test]
    fn add_postfix() {
        let mut t = Fixture::new("a--++", &[], &[]);
        t.postfix("decrement", "--");
        t.postfix("increment", "++");
        t.atom("a");

        assert_eq!(
            t.commit().0,
            "increment:e@0..5[decrement:e@0..3[a@0..1 --@1..3] ++@3..5]"
        );
    }

    #[test]
    fn all() {
        let mut t = Fixture::new("!a++*b+c", LEVELS, &[]);
        t.prefix("negate", "!");
        t.postfix("increment", "++");
        t.atom("a");
        t.binary("mul", "*");
        t.atom("b");
        t.binary("add", "+");
        t.atom("c");

        assert_eq!(
            t.commit().0,
            "add:e@0..8[mul:e@0..6[increment:e@0..4[negate:e@0..2[!@0..1 a@1..2] ++@2..4] \
             *@4..5 b@5..6] +@6..7 c@7..8]"
        );
    }

    #[test]
    fn incomplete() {
        let mut t = Fixture::new("!a++*b+c*", LEVELS, &[]);
        t.prefix("negate", "!");
        t.postfix("increment", "++");
        t.atom("a");
        t.binary("mul", "*");
        t.atom("b");
        t.binary("add", "+");
        t.atom("c");
        t.binary("mul", "*");

        assert_eq!(
            t.commit().0,
            "add:e@0..8[mul:e@0..6[increment:e@0..4[negate:e@0..2[!@0..1 a@1..2] ++@2..4] \
             *@4..5 b@5..6] +@6..7 c@7..8]"
        );
    }

    #[test]
    fn add_partial_binary_with_lower_precedence() {
        let mut t = Fixture::new("a+b*c||", LEVELS, &[]);
        t.atom("a");
        t.binary("add", "+");
        t.atom("b");
        t.binary("mul", "*");
        t.atom("c");
        t.binary("bool", "||");

        assert_eq!(
            t.commit(),
            (
                "add:e@0..5[a@0..1 +@1..2 mul:e@2..5[b@2..3 *@3..4 c@4..5]]".to_string(),
                "a+b*c".to_string()
            )
        );
    }

    #[test]
    fn add_partial_binary_with_equal_precedence() {
        let mut t = Fixture::new("a+b*c*", LEVELS, &[]);
        t.atom("a");
        t.binary("add", "+");
        t.atom("b");
        t.binary("mul", "*");
        t.atom("c");
        t.binary("mul", "*");

        assert_eq!(
            t.commit().0,
            "add:e@0..5[a@0..1 +@1..2 mul:e@2..5[b@2..3 *@3..4 c@4..5]]"
        );
    }

    #[test]
    fn add_partial_binary_with_equal_precedence_and_right_associated() {
        let mut t = Fixture::new("a+b*c*", LEVELS, &["mul"]);
        t.atom("a");
        t.binary("add", "+");
        t.atom("b");
        t.binary("mul", "*");
        t.atom("c");
        t.binary("mul", "*");

        assert_eq!(
            t.commit().0,
            "add:e@0..5[a@0..1 +@1..2 mul:e@2..5[b@2..3 *@3..4 c@4..5]]"
        );
    }

    #[test]
    fn mul_partial_binary_with_greater_precedence() {
        let mut t = Fixture::new("a+b*", LEVELS, &[]);
        t.atom("a");
        t.binary("add", "+");
        t.atom("b");
        t.binary("mul", "*");

        assert_eq!(t.commit().0, "add:e@0..3[a@0..1 +@1..2 b@2..3]");
    }

    #[test]
    #[should_panic(expected = "an infix operator is only tried after an operand")]
    fn add_binary_without_an_atom_is_a_bug() {
        let mut t = Fixture::new("+", &[], &[]);
        t.binary("add", "+");
    }

    #[test]
    fn a_single_binary_without_a_trailing_atom_reverts_to_the_atom() {
        let mut t = Fixture::new("a+", &[], &[]);
        t.atom("a");
        t.binary("add", "+");

        assert_eq!(t.commit().0, "a@0..1");
    }

    #[test]
    fn right_associated_nests_right_with_a_tighter_operator_between() {
        let mut t = Fixture::new("aasb+casd", &[("add", 0), ("as", 1)], &["as"]);
        t.atom("a");
        t.binary("as", "as");
        t.atom("b");
        t.binary("add", "+");
        t.atom("c");
        t.binary("as", "as");
        t.atom("d");

        assert_eq!(
            t.commit(),
            (
                "as:e@0..9[a@0..1 as@1..3 as:e@3..9[add:e@3..6[b@3..4 +@4..5 c@5..6] \
                 as@6..8 d@8..9]]"
                    .to_string(),
                "aasb+casd".to_string()
            )
        );
    }

    #[test]
    fn every_kind_of_dangling_binary_reverts_to_the_tree_before_it() {
        // One case per way a binary is added: first, right-associated at
        // the same level, left at the same level, looser (climbs), tighter.
        let cases: &[(&str, &[&str], &str)] = &[
            ("a+", &[], "a@0..1"),
            ("a*b*", &["mul"], "mul:e@0..3[a@0..1 *@1..2 b@2..3]"),
            ("a*b*", &[], "mul:e@0..3[a@0..1 *@1..2 b@2..3]"),
            ("a*b+", &[], "mul:e@0..3[a@0..1 *@1..2 b@2..3]"),
            ("a+b*", &[], "add:e@0..3[a@0..1 +@1..2 b@2..3]"),
        ];
        for &(text, right, expected) in cases {
            let mut t = Fixture::new(text, LEVELS, right);
            let tokens: Vec<String> = text.chars().map(String::from).collect();
            t.atom(&tokens[0]);
            t.binary(if tokens[1] == "*" { "mul" } else { "add" }, &tokens[1]);
            if tokens.len() > 2 {
                t.atom(&tokens[2]);
                t.binary(if tokens[3] == "*" { "mul" } else { "add" }, &tokens[3]);
            }
            assert_eq!(t.commit().0, expected, "{text}");
        }
    }
}
