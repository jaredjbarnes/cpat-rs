//! `[name op value]`: v12's `_isAttributeMatch`, which reads a property of
//! the node by name and compares it with the selector's text using JS
//! operators.

use std::cmp::Ordering;

use super::js::JsValue;
use crate::ast::{EditableTree, NodeRef};

/// The node property an attribute names: v12's `Node` getters. Any other
/// name (a private field, a method, a typo) reads as `undefined` here, which
/// matches no node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Attribute {
    Id,
    Type,
    Name,
    Value,
    FirstIndex,
    LastIndex,
    StartIndex,
    EndIndex,
    Parent,
    Children,
    HasChildren,
    IsLeaf,
    Undefined,
}

impl Attribute {
    pub fn named(name: &str) -> Attribute {
        match name {
            "id" => Attribute::Id,
            "type" => Attribute::Type,
            "name" => Attribute::Name,
            "value" => Attribute::Value,
            "firstIndex" => Attribute::FirstIndex,
            "lastIndex" => Attribute::LastIndex,
            "startIndex" => Attribute::StartIndex,
            "endIndex" => Attribute::EndIndex,
            "parent" => Attribute::Parent,
            "children" => Attribute::Children,
            "hasChildren" => Attribute::HasChildren,
            "isLeaf" => Attribute::IsLeaf,
            _ => Attribute::Undefined,
        }
    }

    /// The property's value, or `None` where v12's reads `null`/`undefined`
    /// (no parent, an unknown name), which no operator matches.
    fn read(self, tree: &EditableTree<'_>, node: NodeRef) -> Option<JsValue> {
        let index = |n: isize| Some(JsValue::Number(n as f64));
        match self {
            Attribute::Id => Some(JsValue::String(tree.id(node).to_string())),
            Attribute::Type => Some(JsValue::String(tree.type_name(node).to_owned())),
            Attribute::Name => Some(JsValue::String(tree.name(node).to_owned())),
            Attribute::Value => Some(JsValue::String(tree.to_string(node))),
            Attribute::FirstIndex | Attribute::StartIndex => index(tree.start_index(node) as isize),
            Attribute::LastIndex => index(tree.last_index(node)),
            Attribute::EndIndex => index(tree.end_index(node) as isize),
            // A node's `toString()` is its value; an array's joins its
            // items' with commas.
            Attribute::Parent => tree
                .parent(node)
                .map(|p| JsValue::Object(tree.to_string(p))),
            Attribute::Children => Some(JsValue::Object(
                tree.children(node)
                    .iter()
                    .map(|&child| tree.to_string(child))
                    .collect::<Vec<_>>()
                    .join(","),
            )),
            Attribute::HasChildren => Some(JsValue::Boolean(tree.has_children(node))),
            Attribute::IsLeaf => Some(JsValue::Boolean(tree.is_leaf(node))),
            Attribute::Undefined => None,
        }
    }
}

/// The selector's operators, by the name the grammar gives each.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Operator {
    Equal,
    NotEqual,
    StartsWith,
    EndsWith,
    Contains,
    GreaterThanOrEqual,
    LessThanOrEqual,
    GreaterThan,
    LessThan,
}

impl Operator {
    /// The operator a selector node names, if it names one.
    pub fn named(name: &str) -> Option<Operator> {
        Some(match name {
            "equal" => Operator::Equal,
            "not-equal" => Operator::NotEqual,
            "starts-with" => Operator::StartsWith,
            "ends-with" => Operator::EndsWith,
            "contains" => Operator::Contains,
            "greater-than-or-equal" => Operator::GreaterThanOrEqual,
            "less-than-or-equal" => Operator::LessThanOrEqual,
            "greater-than" => Operator::GreaterThan,
            "less-than" => Operator::LessThan,
            _ => return None,
        })
    }

    fn holds(self, property: &JsValue, text: &str) -> bool {
        let ordering = || property.compare(text);
        match self {
            Operator::Equal => property.strict_equals(text),
            Operator::NotEqual => !property.strict_equals(text),
            Operator::StartsWith => property.to_js_string().starts_with(text),
            Operator::EndsWith => property.to_js_string().ends_with(text),
            Operator::Contains => property.to_js_string().contains(text),
            Operator::GreaterThanOrEqual => {
                matches!(ordering(), Some(Ordering::Greater | Ordering::Equal))
            }
            Operator::LessThanOrEqual => {
                matches!(ordering(), Some(Ordering::Less | Ordering::Equal))
            }
            Operator::GreaterThan => ordering() == Some(Ordering::Greater),
            Operator::LessThan => ordering() == Some(Ordering::Less),
        }
    }
}

/// One attribute test, compiled from a selector.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct AttributeTest {
    pub attribute: Attribute,
    pub operator: Operator,
    /// The selector's text, quotes removed. It stays text: JS converts it
    /// only when the property it meets is not a string.
    pub value: String,
}

impl AttributeTest {
    pub fn matches(&self, tree: &EditableTree<'_>, node: NodeRef) -> bool {
        self.attribute
            .read(tree, node)
            .is_some_and(|property| self.operator.holds(&property, &self.value))
    }
}
