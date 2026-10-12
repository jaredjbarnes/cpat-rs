//! `( p, divider trim )` with `+`, `*`, `{n}`, `{m,}`, `{,n}` or `{m,n}`
//! (v12 `Grammar._buildRepeat`).

use crate::pattern::Pattern;
use crate::patterns::{Optional, Repeat, RepeatOptions};
use crate::tree::Node;

use super::builder::Builder;
use super::error::GrammarError;
use super::syntax::is_skipped;

/// How many times, read from the quantifier.
enum Count {
    /// `+`
    OneOrMore,
    /// `*`: an optional one-or-more, as v12 builds it.
    ZeroOrMore,
    /// `{…}`
    Bounds { min: usize, max: Option<usize> },
}

impl<'a, 't> Builder<'a, 't> {
    pub(super) fn build_repeat(
        &mut self,
        name: &str,
        node: Node<'a, 't>,
    ) -> Result<Pattern, GrammarError> {
        // Every part is a direct child of the repeat. v12 searches the
        // whole subtree for the divider and the quantifier, so a repeat
        // whose item is itself a repeat took the item's (a deviation fixed
        // here, DESIGN §18).
        let item = node
            .children()
            .find(|child| {
                !is_skipped(child)
                    && !matches!(
                        child.name(),
                        "(" | ")" | "delimiter" | "oneOrMore" | "zeroOrMore" | "repeatBounds"
                    )
            })
            .expect("the cpat grammar gives every repeat an item");
        let item = self.build_part(item)?;

        let mut options = RepeatOptions::default();
        if let Some(delimiter) = node.children().find(|child| child.name() == "delimiter") {
            let divider = delimiter
                .children()
                .find(|child| !is_skipped(child) && !matches!(child.name(), "," | "trim"));
            if let Some(divider) = divider {
                options.divider = Some(self.build_part(divider)?);
            }
            options.trim_divider = delimiter.children().any(|child| child.name() == "trim");
        }

        let quantifier = node
            .children()
            .last()
            .expect("the cpat grammar ends every repeat with a quantifier");
        match count(quantifier) {
            Count::OneOrMore => Ok(Repeat::new(name, item, options).into()),
            Count::ZeroOrMore => {
                let repeat = Repeat::new(&format!("inner-optional-{name}"), item, options);
                Ok(Optional::new(name, repeat.into()).into())
            }
            Count::Bounds { min, max } => {
                options.min = min;
                options.max = max;
                Ok(Repeat::new(name, item, options).into())
            }
        }
    }
}

fn count(quantifier: Node<'_, '_>) -> Count {
    match quantifier.name() {
        "oneOrMore" => Count::OneOrMore,
        "zeroOrMore" => Count::ZeroOrMore,
        _ => bounds(quantifier),
    }
}

/// `{n}`, `{m,}`, `{,n}`, `{m,n}`. With no number at all (`{}` or `{,}`)
/// v12 keeps one-or-more.
fn bounds(node: Node<'_, '_>) -> Count {
    let integers: Vec<Node<'_, '_>> = node
        .children()
        .filter(|child| child.name() == "integer")
        .collect();
    let comma = node.children().find(|child| child.name() == ",");

    match (integers.as_slice(), comma) {
        ([min, max], _) => Count::Bounds {
            min: number(min),
            max: Some(number(max)),
        },
        ([only], Some(comma)) if only.start() < comma.start() => Count::Bounds {
            min: number(only),
            max: None,
        },
        ([only], Some(_)) => Count::Bounds {
            min: 0,
            max: Some(number(only)),
        },
        ([only], None) => Count::Bounds {
            min: number(only),
            max: Some(number(only)),
        },
        _ => Count::OneOrMore,
    }
}

/// An `integer` node's value. One too big for `usize` is as good as
/// unbounded, and a repeat's slot count saturates anyway (DESIGN §10).
fn number(node: &Node<'_, '_>) -> usize {
    node.value().parse().unwrap_or(usize::MAX)
}
