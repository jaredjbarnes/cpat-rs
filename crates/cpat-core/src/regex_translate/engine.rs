use super::Dialect;
use super::ast::{Assertion, GroupKind, Node};
use super::mode::Mode;

/// The linear-time engine unless the source uses something only the
/// backtracking one can match: lookaround and backreferences, and what this
/// translation spells with lookaround (`^`/`$` under `m`, `\b`/`\B` under `i`).
pub(super) fn dialect(root: &Node) -> Dialect {
    match needs_backtracking(root, Mode::default()) {
        true => Dialect::Fancy,
        false => Dialect::Fast,
    }
}

fn needs_backtracking(node: &Node, mode: Mode) -> bool {
    match node {
        Node::Look(_) | Node::Backreference(_) => true,
        Node::Assertion(Assertion::Start | Assertion::End) => mode.multiline,
        Node::Assertion(Assertion::WordBoundary | Assertion::NotWordBoundary) => mode.ignore_case,
        Node::Alternation(items) | Node::Sequence(items) => {
            items.iter().any(|item| needs_backtracking(item, mode))
        }
        Node::Group(group) => {
            let inner = match group.kind {
                GroupKind::Modifiers(flags) => mode.with(flags),
                GroupKind::Capture(_) | GroupKind::NonCapture => mode,
            };
            needs_backtracking(&group.body, inner)
        }
        Node::Repeat(repeat) => needs_backtracking(&repeat.body, mode),
        Node::Char(_) | Node::Dot | Node::Set(_) | Node::Class(_) => false,
    }
}
