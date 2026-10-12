#![allow(dead_code)]

use cpat_core::{
    Choice, Context, Cursor, Cut, Literal, Node, NodeKind, Not, Optional, ParseError, Pattern,
    Reference, Regex, Repeat, RepeatOptions, RightAssociated, Sequence,
};

pub fn lit(name: &str, token: &str) -> Pattern {
    Literal::new(name, token).unwrap().into()
}

pub fn re(name: &str, source: &str) -> Pattern {
    Regex::new(name, source).unwrap().into()
}

pub fn seq(name: &str, children: Vec<Pattern>) -> Pattern {
    Sequence::new(name, children).unwrap().into()
}

pub fn choice(name: &str, alternatives: Vec<Pattern>) -> Pattern {
    Choice::new(name, alternatives).unwrap().into()
}

pub fn opt(name: &str, child: Pattern) -> Pattern {
    Optional::new(name, child).into()
}

pub fn not(name: &str, child: Pattern) -> Pattern {
    Not::new(name, child).into()
}

pub fn cut() -> Pattern {
    Cut::default().into()
}

pub fn rep(name: &str, child: Pattern, options: RepeatOptions) -> Pattern {
    Repeat::new(name, child, options).into()
}

/// `RepeatOptions` with a divider; the rest default.
pub fn divided(divider: Pattern) -> RepeatOptions {
    RepeatOptions {
        divider: Some(divider),
        ..RepeatOptions::default()
    }
}

pub fn reference(name: &str) -> Pattern {
    Reference::new(name).into()
}

pub fn reference_to(name: &str, target: &str) -> Pattern {
    Reference::to(name, target).into()
}

pub fn context(name: &str, child: Pattern, patterns: Vec<Pattern>) -> Pattern {
    Context::new(name, child, patterns).into()
}

pub fn right(child: Pattern) -> Pattern {
    RightAssociated::new(child).into()
}

/// The `Choice` inside a pattern built as one.
pub fn as_choice(pattern: &Pattern) -> &Choice {
    match pattern {
        Pattern::Choice(choice) => choice,
        other => panic!("not a choice: {other:?}"),
    }
}

/// A tree as one line with kinds, `name:kind@start..end[children]`, for
/// assertions on exact shapes (the v12 comparison printed the same, with
/// `type` for kind).
pub fn shape(node: Node<'_, '_>) -> String {
    let children: Vec<String> = node.children().map(shape).collect();
    let children = if children.is_empty() {
        String::new()
    } else {
        format!("[{}]", children.join(" "))
    };
    let kind = match node.kind() {
        NodeKind::Literal => "literal",
        NodeKind::Regex => "regex",
        NodeKind::Sequence => "sequence",
        NodeKind::InfiniteRepeat => "infinite-repeat",
        NodeKind::FiniteRepeat => "finite-repeat",
        NodeKind::Custom => "custom",
        NodeKind::Expression => "expression",
    };
    format!(
        "{}:{kind}@{}..{}{children}",
        node.name(),
        node.start(),
        node.end()
    )
}

/// `exec`'s tree as a `shape`, or `-`.
pub fn exec_shape(pattern: &mut Pattern, text: &str) -> String {
    let result = pattern.exec(text);
    assert!(result.pattern_error.is_none(), "{:?}", result.pattern_error);
    result
        .tree()
        .map_or_else(|| "-".to_string(), |tree| shape(tree.root()))
}

/// `parse` from 0 (not necessarily all of `text`): the tree as a `shape`
/// or `-`, then the cursor's error and furthest error.
pub fn parse_shape(pattern: &mut Pattern, text: &str) -> (String, String, String) {
    let mut cursor = Cursor::new(text);
    let node = pattern.parse(&mut cursor);
    let error = error_outline(cursor.error());
    let furthest = error_outline(cursor.furthest_error());
    let tree = node.map_or_else(
        || "-".to_string(),
        |node| shape(cursor.into_tree(node).root()),
    );
    (tree, error, furthest)
}

/// A tree as one line, `name@start..end[children]`: the same shape the v12
/// comparison script printed, so expected values are pasted from its output
/// (TS inclusive `lastIndex` L printed as end L+1).
pub fn outline(node: Node<'_, '_>) -> String {
    let children: Vec<String> = node.children().map(outline).collect();
    let children = if children.is_empty() {
        String::new()
    } else {
        format!("[{}]", children.join(" "))
    };
    format!(
        "{}@{}..{}{}",
        node.name(),
        node.start(),
        node.end(),
        children
    )
}

/// An error as `name@start..last`, or `-`.
pub fn error_outline(error: Option<ParseError>) -> String {
    error.map_or_else(
        || "-".to_string(),
        |e| format!("{}@{}..{}", e.pattern.name, e.start, e.last),
    )
}

/// `exec` summarised as `(tree, error, furthest error)` outlines.
pub fn exec_outline(pattern: &mut Pattern, text: &str) -> (String, String, String) {
    let result = pattern.exec(text);
    assert!(result.pattern_error.is_none(), "{:?}", result.pattern_error);
    (
        result
            .tree()
            .map_or_else(|| "-".to_string(), |tree| outline(tree.root())),
        error_outline(result.error),
        error_outline(result.furthest_error),
    )
}

pub fn outlined(tree: &str, error: &str, furthest: &str) -> (String, String, String) {
    (tree.to_string(), error.to_string(), furthest.to_string())
}
