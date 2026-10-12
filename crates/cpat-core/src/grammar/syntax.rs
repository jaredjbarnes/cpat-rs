//! Reading the cpat grammar's trees: what a node is, which nodes are only
//! punctuation, and v12's two tree searches the builder relies on.

use crate::tree::Node;

/// What a node of a rule's right-hand side builds, decided from its name
/// once, so the builder matches on a closed set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Syntax {
    Literal,
    Regex,
    Sequence,
    Options,
    Repeat,
    Group,
    Not,
    Optional,
    RightAssociation,
    Identifier,
    TakeUntil,
    Block,
    BlockDelimiter,
    /// Nothing the builder knows how to build.
    Other,
}

impl Syntax {
    pub(super) fn of(node: &Node<'_, '_>) -> Syntax {
        match node.name() {
            "literal" => Syntax::Literal,
            "regex" => Syntax::Regex,
            "sequenceExpr" => Syntax::Sequence,
            "optionsExpr" => Syntax::Options,
            "repeatExpr" => Syntax::Repeat,
            "patternGroupExpr" => Syntax::Group,
            "notExpr" => Syntax::Not,
            "optionalExpr" => Syntax::Optional,
            "rightAssociationExpr" => Syntax::RightAssociation,
            "patternIdentifier" => Syntax::Identifier,
            "takeUntilExpr" => Syntax::TakeUntil,
            "blockExpr" => Syntax::Block,
            "blockDelimiter" => Syntax::BlockDelimiter,
            _ => Syntax::Other,
        }
    }
}

/// v12's `skipNodes`: operators and whitespace, which build nothing.
/// (`optionalWS`/`optionalLS` never name a node, since an optional's node
/// is its child's; v12 lists them anyway.)
pub(super) fn is_skipped(node: &Node<'_, '_>) -> bool {
    matches!(
        node.name(),
        "+" | "|" | "optionalWS" | "optionalLS" | "ws" | "ls"
    )
}

/// The children of a `sequenceExpr` or `optionsExpr` node with every
/// nested node of the same kind replaced by its own children (DESIGN §8):
/// `a + b + c` parses as `(a + b) + c`, and builds one sequence of three.
/// v12 rewrites the tree to the same effect before building
/// (`_flattenExpressionsRecursive`); here the tree is read-only, so the
/// flattening happens as the children are read. Iterative: a chain of
/// operators is as deep as it is long.
pub(super) fn flattened<'a, 't>(node: Node<'a, 't>) -> Vec<Node<'a, 't>> {
    let name = node.name();
    let mut flat = Vec::new();
    let mut pending: Vec<Node<'a, 't>> = node.children().rev().collect();
    while let Some(child) = pending.pop() {
        if child.name() == name {
            pending.extend(child.children().rev());
        } else {
            flat.push(child);
        }
    }
    flat
}

/// v12's `Node.find`, quirk included: the walk goes children first and
/// stops at the first match, but every ancestor of it (up to `node`) is
/// still offered to the predicate on the way out, and the last one that
/// matches wins. So for a left-nested chain of matches, the outermost
/// is found. The builder relies on this only to name a removed `<|>`
/// expression in full (`a <|> b <|> c` nests as `(a <|> b) <|> c`).
pub(super) fn find_outermost<'a, 't>(
    node: Node<'a, 't>,
    predicate: impl Fn(&Node<'a, 't>) -> bool,
) -> Option<Node<'a, 't>> {
    let first = node.find(&predicate)?;
    let mut found = first;
    let mut current = first;
    while current.id() != node.id() {
        let Some(parent) = current.parent() else {
            break;
        };
        if predicate(&parent) {
            found = parent;
        }
        current = parent;
    }
    Some(found)
}

/// The 1-based line and column of byte `at` in `text`, as v12 computes
/// them for the removed-syntax message: the column counts UTF-16 units.
pub(super) fn line_and_column(text: &str, at: usize) -> (usize, usize) {
    let before = &text[..at];
    let line = before.matches('\n').count() + 1;
    let last_line = before.rsplit('\n').next().unwrap_or("");
    (line, last_line.encode_utf16().count() + 1)
}
