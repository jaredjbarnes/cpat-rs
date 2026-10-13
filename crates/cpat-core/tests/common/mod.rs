#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::Rc;

use cpat_core::{
    Block, Choice, Context, Cursor, Custom, CustomParse, Cut, Handle, Literal, Meta, Node, NodeId,
    NodeKind, Not, Optional, ParseError, Pattern, PatternId, PatternPath, Reference, Regex, Repeat,
    RepeatOptions, RightAssociated, Sequence, TakeUntil, Tokenizable,
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

pub fn take_until(name: &str, terminator: Pattern) -> Pattern {
    TakeUntil::new(name, terminator).into()
}

/// A block with literal delimiters `open` and `close` (named `open` and
/// `close`, as in v12's tests).
pub fn block(name: &str, open: &str, content: Option<Pattern>, close: &str) -> Pattern {
    Block::new(
        name,
        Literal::new("open", open).unwrap(),
        content,
        Literal::new("close", close).unwrap(),
    )
    .into()
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
        NodeKind::TakeUntil => "take-until",
        NodeKind::Block => "block",
        NodeKind::BlockContent => "block-content",
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

/// What a `frame_spy` saw: the ancestor frames, `(id, start)`, at each call.
pub type SeenFrames = Rc<RefCell<Vec<Vec<(PatternId, usize)>>>>;

/// A custom pattern that notes the ancestor frames around it, then
/// succeeds with no node and no error.
pub fn frame_spy() -> (Pattern, SeenFrames) {
    #[derive(Clone)]
    struct Spy(SeenFrames);

    impl CustomParse for Spy {
        fn parse(&mut self, _meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
            let frames = cursor.ancestors().iter().map(|f| (f.id, f.start)).collect();
            self.0.borrow_mut().push(frames);
            None
        }

        fn clone_box(&self) -> Box<dyn CustomParse> {
            Box::new(self.clone())
        }
    }

    let seen = SeenFrames::default();
    let spy = Custom::new("spy", Box::new(Spy(Rc::clone(&seen))));
    (spy.into(), seen)
}

/// A regex with intellisense tokens set.
pub fn re_tokens(name: &str, source: &str, tokens: &[&str]) -> Pattern {
    let mut regex = Regex::new(name, source).unwrap();
    regex.set_tokens(tokens.iter().map(|t| t.to_string()).collect());
    regex.into()
}

/// A `PatternPath` from its steps.
pub fn at(steps: &[u32]) -> PatternPath {
    PatternPath::new(steps)
}

/// The names of introspection results, in order.
pub fn names(handles: &[Handle<'_>]) -> Vec<&'static str> {
    handles.iter().map(Handle::name).collect()
}

/// The paths of introspection results, in order, as `/0/1` strings.
pub fn paths(handles: &[Handle<'_>]) -> Vec<String> {
    handles.iter().map(|h| format!("{:?}", h.path)).collect()
}

/// The path of the first pattern named `name` in `root` (v12
/// `root.find(p => p.name === name)`).
pub fn path_of(root: &Pattern, name: &str) -> PatternPath {
    root.find(|p| p.name() == name)
        .unwrap_or_else(|| panic!("no pattern named {name}"))
        .path
}

/// The tree v12's `selector.test.ts` and `query.test.ts` build: a root, two
/// grand-parents, three parents each, four children each valued 1 to 24,
/// normalized so positions run end to end.
pub fn family_tree(tree: &mut cpat_core::EditableTree<'static>) -> cpat_core::NodeRef {
    let mut value = 0;
    let mut grand_parents = Vec::new();
    for _ in 0..2 {
        let mut parents = Vec::new();
        for _ in 0..3 {
            let children: Vec<_> = (0..4)
                .map(|_| {
                    value += 1;
                    tree.create_value_node("node", "child", value.to_string())
                })
                .collect();
            parents.push(tree.create_node("node", "parent", &children));
        }
        grand_parents.push(tree.create_node("node", "grand-parent", &parents));
    }
    let root = tree.create_node("node", "root", &grand_parents);
    tree.normalize(root);
    root
}

/// The nodes' values joined with commas, as v12's tests print them.
pub fn joined(tree: &cpat_core::EditableTree<'_>, nodes: &[cpat_core::NodeRef]) -> String {
    nodes
        .iter()
        .map(|&n| tree.to_string(n))
        .collect::<Vec<_>>()
        .join(",")
}
