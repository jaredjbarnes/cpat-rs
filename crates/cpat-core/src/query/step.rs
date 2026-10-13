//! A selector's syntax tree, compiled into the steps v12's `_process` takes.
//!
//! v12 walks the selector's tree on every call (`walkUp`, children before
//! parents) and decides at each node what to do from its name and its
//! parent's name. That decision depends only on the selector, so here it is
//! made once, in the same order, and a call just runs the list.

use super::attribute::{Attribute, AttributeTest, Operator};
use super::combinator::Combinator;
use super::error::SelectorError;
use super::selector::Selector;
use crate::tree::{Node, Tree};

#[derive(Clone, Debug)]
pub(crate) enum Step {
    /// `*`: every node the combinator reaches.
    WildCard,
    /// `a, b`: each part run on the current selection, the results joined
    /// without repeats. v12 parses each part again as its own selector.
    Or(Vec<Selector>),
    /// `child`: nodes with that name.
    Name(String),
    /// `child[value='4']`: nodes with that name that pass the test.
    NameWithAttribute { name: String, test: AttributeTest },
    /// `[value='4']`.
    Attribute(AttributeTest),
    /// Where the next step looks. A finished `a > b` sets it back to
    /// `Descendant`, which is what v12's `null` does.
    Combinator(Combinator),
}

/// Every step, in v12's order. Only an or-selector's parts can fail (they
/// are parsed again), and they can't in practice: each is text the grammar
/// just accepted as a node selector.
pub(crate) fn compile(tree: &Tree<'_>) -> Result<Vec<Step>, SelectorError> {
    let mut steps = Vec::new();
    let mut failure = None;
    tree.root().walk_up(|node| {
        match step_for(node) {
            Ok(Some(step)) => steps.push(step),
            Ok(None) => {}
            Err(error) => {
                failure.get_or_insert(error);
            }
        }
        true
    });
    match failure {
        Some(error) => Err(error),
        None => Ok(steps),
    }
}

/// v12's `_process`, kept quirk for quirk: a `name` or `attribute-selector`
/// acts only at the top or directly under a `selector-expression`, while a
/// `name-selector` or `*` acts wherever it is, even inside an or-selector,
/// where it narrows the selection before the or-selector runs.
fn step_for(node: Node<'_, '_>) -> Result<Option<Step>, SelectorError> {
    let at_top = node
        .parent()
        .is_none_or(|parent| parent.name() == "selector-expression");
    let step = match node.name() {
        "wild-card" => Step::WildCard,
        "or-selector" => Step::Or(
            node.children()
                .filter(|part| part.name() != "comma")
                .map(|part| Selector::new(part.value()))
                .collect::<Result<_, _>>()?,
        ),
        "name-selector" => name_selector(node),
        "name" if at_top => Step::Name(node.value().to_owned()),
        "attribute-selector" if at_top => Step::Attribute(attribute_test(node)),
        "selector-expression" => Step::Combinator(Combinator::Descendant),
        name => match Combinator::named(name) {
            Some(combinator) => Step::Combinator(combinator),
            None => return Ok(None),
        },
    };
    Ok(Some(step))
}

/// `name[attribute]`. Chained (`a[x='1'][y='2']`) it nests to the left, and
/// v12 then takes the name as the inner selector's whole text and the test
/// from the first attribute parts it finds: both quirks are kept.
fn name_selector(node: Node<'_, '_>) -> Step {
    let mut children = node.children();
    match (children.next(), children.len()) {
        (Some(first), 1..) => Step::NameWithAttribute {
            name: first.value().to_owned(),
            test: attribute_test(node),
        },
        _ => Step::Name(node.value().to_owned()),
    }
}

/// v12's `_getAttributeName`, `_getAttributeOperator` and
/// `_getAttributeValue`, each a `find` over the selector node (v12's `find`:
/// the first match children first, unless an ancestor of it matches too).
fn attribute_test(node: Node<'_, '_>) -> AttributeTest {
    const BUILT: &str = "the grammar gives every attribute selector this part";
    let named = |name: &str| node.find(|n| n.name() == name);

    let attribute = named("attribute-name").expect(BUILT).value();
    let operator = node
        .find(|n| Operator::named(n.name()).is_some())
        .and_then(|n| Operator::named(n.name()))
        .expect(BUILT);
    let value = match named("single-quote-string-literal") {
        // Quotes are one byte each.
        Some(quoted) => &quoted.value()[1..quoted.value().len() - 1],
        None => named("value")
            .or_else(|| named("number"))
            .expect(BUILT)
            .value(),
    };

    AttributeTest {
        attribute: Attribute::named(attribute),
        operator,
        value: value.to_owned(),
    }
}
