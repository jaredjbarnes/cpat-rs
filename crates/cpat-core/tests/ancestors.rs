//! The cursor's ancestor stack (DESIGN §2): which composites are parsing,
//! and where each began. A `Custom` pattern records what it sees.

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::*;
use cpat_core::{
    Context, Cursor, Custom, CustomParse, Frame, Meta, NodeId, NodeKind, Pattern, PatternId,
};

/// Matches one character and records the ancestor frames it saw.
#[derive(Clone)]
struct Witness {
    seen: Rc<RefCell<Vec<Vec<Frame>>>>,
}

impl CustomParse for Witness {
    fn parse(&mut self, meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        self.seen.borrow_mut().push(cursor.ancestors().to_vec());
        let start = cursor.pos();
        let end = cursor.snap_forward(start + 1);
        if start == end {
            cursor.record_error(start, start, meta);
            return None;
        }
        cursor.set_pos(end);
        Some(cursor.leaf(NodeKind::Custom, meta.name, start, end))
    }

    fn clone_box(&self) -> Box<dyn CustomParse> {
        Box::new(self.clone())
    }
}

fn witness() -> (Pattern, Rc<RefCell<Vec<Vec<Frame>>>>) {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let pattern = Custom::new("w", Box::new(Witness { seen: seen.clone() })).into();
    (pattern, seen)
}

fn frames(of: &[(PatternId, usize)]) -> Vec<Frame> {
    of.iter()
        .map(|&(id, start)| Frame {
            id,
            start,
            child: 0,
        })
        .collect()
}

#[test]
fn composites_push_a_frame_while_their_children_parse() {
    let (w, seen) = witness();
    let optional = opt("o", w);
    let o = optional.id();
    let mut pattern = seq("s", vec![lit("a", "a"), optional]);
    let s = pattern.id();
    let mut cursor = Cursor::new("ab");

    assert!(pattern.parse(&mut cursor).is_some());

    assert_eq!(*seen.borrow(), vec![frames(&[(s, 0), (o, 1)])]);
    assert!(cursor.ancestors().is_empty(), "every frame is popped");
}

#[test]
fn choices_repeats_and_lookaheads_push_frames_too() {
    let (w, seen) = witness();
    let lookahead = not("n", lit("z", "z"));
    let alternative = seq("alt", vec![lookahead, w]);
    let alt = alternative.id();
    let either = choice("c", vec![alternative]);
    let c = either.id();
    let mut repeat = rep("r", either, Default::default());
    let r = repeat.id();

    assert!(repeat.test("xy"));

    assert_eq!(
        *seen.borrow(),
        vec![
            frames(&[(r, 0), (c, 0), (alt, 0)]),
            frames(&[(r, 0), (c, 1), (alt, 1)]),
        ]
    );
}

#[test]
fn references_contexts_and_resolved_targets_push_frames() {
    // The resolved target is a clone of the master, so its frame carries the
    // master's id: that is what the recursion guards compare.
    let (w, seen) = witness();
    let target = seq("t", vec![w]);
    let t = target.id();
    let link = reference("t");
    let r = link.id();
    let body = seq("s", vec![link]);
    let s = body.id();
    let mut root: Pattern = Context::new("c", body, vec![target]).into();
    let c = root.id();

    assert!(root.test("x"));

    assert_eq!(
        *seen.borrow(),
        vec![frames(&[(c, 0), (s, 0), (r, 0), (t, 0)])]
    );
}

#[test]
fn the_stack_is_empty_after_a_failed_parse() {
    let (w, _) = witness();
    let mut pattern = seq("s", vec![opt("o", seq("in", vec![w, lit("q", "q")]))]);
    let mut cursor = Cursor::new("ab");

    pattern.parse(&mut cursor);

    assert!(cursor.ancestors().is_empty());
}

#[test]
fn a_lookahead_pushes_a_frame() {
    let (w, seen) = witness();
    let lookahead = not("n", w);
    let n = lookahead.id();
    let mut pattern = seq("s", vec![lookahead, lit("z", "z")]);
    let s = pattern.id();

    // `w` matches the "z", so `!w` fails; what matters is what it saw.
    assert!(!pattern.test("z"));

    assert_eq!(*seen.borrow(), vec![frames(&[(s, 0), (n, 0)])]);
}

#[test]
fn the_lookaheads_frame_feeds_the_choice_guard() {
    // Inside `!c`, the choice `c` is already parsing at 0, so the guard
    // skips it and the reference to `c` fails there, at `n@0..0`. Without
    // the lookahead's frame the error would come from deeper inside
    // (`c@0..0`); v12 reports `n@0..0`.
    let mut pattern = seq(
        "root",
        vec![
            choice("c", vec![not("n", reference("c")), lit("x", "x")]),
            lit("x2", "x"),
        ],
    );

    assert_eq!(
        exec_outline(&mut pattern, "xx"),
        outlined("root@0..2[x@0..1 x2@1..2]", "-", "n@0..0")
    );
    assert_eq!(
        exec_outline(&mut pattern, "y"),
        outlined("-", "root@0..1", "x@0..0")
    );
}

#[test]
fn a_right_associated_marker_pushes_no_frame() {
    // Not a link in the pattern tree (v12 doesn't make it its child's
    // parent), so the child's recursion guards look straight past it.
    let (w, seen) = witness();
    let mut pattern = seq("s", vec![right(w)]);
    let s = pattern.id();

    pattern.parse(&mut Cursor::new("x"));

    assert_eq!(*seen.borrow(), vec![frames(&[(s, 0)])]);
}

#[test]
fn an_operator_of_a_pratt_choice_parses_inside_its_own_frame() {
    // v12 makes the choice the operator sequence's parent, so the
    // operator is on the chain below the choice.
    let (w, seen) = witness();
    let mut expr = choice(
        "expr",
        vec![seq("neg", vec![w, reference("expr")]), lit("a", "a")],
    );
    let choice_id = expr.id();
    let Pattern::Choice(classified) = &expr else {
        unreachable!()
    };
    let operator = classified.prefix_patterns().next().unwrap();
    assert_eq!(operator.name(), "neg-prefix");
    let neg = cpat_core::Pattern::from(operator.clone()).id();

    expr.parse(&mut Cursor::new("-a"));

    assert_eq!(seen.borrow()[0], frames(&[(choice_id, 0), (neg, 0)]));
}
