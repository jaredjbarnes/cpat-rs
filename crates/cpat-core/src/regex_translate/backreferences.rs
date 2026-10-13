//! Finds the backreferences whose JS meaning no Rust engine reproduces.
//!
//! JS clears the captures inside a quantified atom each time it repeats,
//! and drops an iteration that matched nothing; `fancy-regex` keeps the
//! previous iteration's capture. So `(?:(a)|b)+\1` sees an `a` from an
//! earlier round in Rust and nothing in JS. Where every round sets the group
//! before the backreference reads it, the two agree; everywhere else the
//! source is refused rather than matched differently.

use super::ast::{GroupKind, Node};
use super::error::TranslateError::{self, Unsupported};

pub(super) fn check(root: &Node) -> Result<(), TranslateError> {
    let mut found = Found::default();
    collect(root, &mut Vec::new(), &mut found);

    for (reference, ancestors, at) in &found.references {
        if ancestors
            .iter()
            .any(|a| matches!(a, Node::Look(look) if look.behind))
        {
            return Err(Unsupported(
                "a backreference inside a lookbehind (JavaScript matches lookbehinds backwards)",
            ));
        }
        for group in reference.iter() {
            let (_, group_ancestors, end) = found
                .groups
                .iter()
                .find(|(index, ..)| index == group)
                .expect("the parser checked every backreference's group exists");
            let inside_group = ancestors.iter().any(|a| is_capture(a, *group));
            if inside_group {
                // Always empty in JS; the emitter writes nothing for it.
                continue;
            }
            for (body, repeat) in group_ancestors.iter().copied().filter_map(repeat_of) {
                let encloses_reference = ancestors.iter().any(|a| std::ptr::eq(*a, repeat));
                let agrees = mandatory(body, *group)
                    && !body.nullable()
                    && (!encloses_reference || at > end);
                if !agrees {
                    return Err(Unsupported(
                        "a backreference to a group a repetition may skip or reset (JavaScript clears it each round, Rust keeps the last round's)",
                    ));
                }
            }
        }
    }
    Ok(())
}

#[derive(Default)]
struct Found<'a> {
    /// (group, its ancestors, where it closed)
    groups: Vec<(usize, Vec<&'a Node>, usize)>,
    /// (the groups it may read, its ancestors, where it was read)
    references: Vec<(&'a [usize], Vec<&'a Node>, usize)>,
}

fn collect<'a>(node: &'a Node, ancestors: &mut Vec<&'a Node>, found: &mut Found<'a>) {
    let children: Vec<&Node> = match node {
        Node::Alternation(items) | Node::Sequence(items) => items.iter().collect(),
        Node::Group(group) => {
            if let GroupKind::Capture(index) = group.kind {
                found.groups.push((index, ancestors.clone(), group.end));
            }
            vec![&group.body]
        }
        Node::Look(look) => vec![&look.body],
        Node::Repeat(repeat) => vec![&repeat.body],
        Node::Backreference(reference) => {
            found
                .references
                .push((&reference.groups, ancestors.clone(), reference.at));
            vec![]
        }
        Node::Char(_) | Node::Dot | Node::Set(_) | Node::Class(_) | Node::Assertion(_) => vec![],
    };
    ancestors.push(node);
    for child in children {
        collect(child, ancestors, found);
    }
    ancestors.pop();
}

/// A repeat whose body can run more than once: its body, and the node.
fn repeat_of(node: &Node) -> Option<(&Node, &Node)> {
    match node {
        Node::Repeat(repeat) if repeat.repeats() => Some((&repeat.body, node)),
        _ => None,
    }
}

fn is_capture(node: &Node, index: usize) -> bool {
    matches!(node, Node::Group(group) if matches!(group.kind, GroupKind::Capture(i) if i == index))
}

/// Whether every match of `node` passes through capture `index`.
fn mandatory(node: &Node, index: usize) -> bool {
    match node {
        Node::Group(group) => is_capture(node, index) || mandatory(&group.body, index),
        Node::Sequence(items) => items.iter().any(|item| mandatory(item, index)),
        Node::Repeat(repeat) => repeat.min > 0 && mandatory(&repeat.body, index),
        Node::Look(look) => !look.negated && mandatory(&look.body, index),
        Node::Alternation(_)
        | Node::Char(_)
        | Node::Dot
        | Node::Set(_)
        | Node::Class(_)
        | Node::Assertion(_)
        | Node::Backreference(_) => false,
    }
}
