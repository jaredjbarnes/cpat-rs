//! v12's three walks, each with its own stack (§7e). The visitor gets the
//! tree back with each node, so a walk over an `EditableTree` may edit it:
//! as in v12, a node's children are read before it is visited (`walk_up`,
//! `walk_down`) or after (`walk_breadth_first`), and an edit changes only
//! what is read later.

use std::collections::VecDeque;

use super::view::AstView;

/// Children before their parent. A `false` from a node skips its remaining
/// siblings, and every ancestor still gets its visit, as v12's
/// `every(…) && callback(this)` does: the callback runs first, so it is
/// never short-circuited away. Returns `false` if any visit did.
pub(crate) fn walk_up<V: AstView>(
    view: &mut V,
    from: V::Ref,
    mut visit: impl FnMut(&mut V, V::Ref) -> bool,
) -> bool {
    struct Frame<R> {
        node: R,
        // This node's children as they were when it was entered, kept in
        // `snapshot[at..at + len]`. Frames end in stack order, so one shared
        // list truncated on the way out holds every frame's copy.
        at: usize,
        len: usize,
        next: usize,
        all_true: bool,
    }

    let mut snapshot = Vec::new();
    let enter = |view: &V, node: V::Ref, snapshot: &mut Vec<V::Ref>| {
        let at = snapshot.len();
        snapshot.extend_from_slice(view.children_of(node));
        Frame {
            node,
            at,
            len: snapshot.len() - at,
            next: 0,
            all_true: true,
        }
    };

    let mut frames = vec![enter(view, from, &mut snapshot)];
    loop {
        let top = frames.last_mut().expect("a frame until the root returns");
        if top.all_true && top.next < top.len {
            let child = snapshot[top.at + top.next];
            top.next += 1;
            let frame = enter(view, child, &mut snapshot);
            frames.push(frame);
            continue;
        }

        let frame = frames.pop().expect("checked above");
        snapshot.truncate(frame.at);
        let result = visit(view, frame.node) && frame.all_true;
        match frames.last_mut() {
            Some(parent) => parent.all_true = result,
            None => return result,
        }
    }
}

/// A parent before its children. The first `false` ends the whole walk.
pub(crate) fn walk_down<V: AstView>(
    view: &mut V,
    from: V::Ref,
    mut visit: impl FnMut(&mut V, V::Ref) -> bool,
) -> bool {
    let mut pending = vec![from];
    let mut children = Vec::new();
    while let Some(node) = pending.pop() {
        children.clear();
        children.extend_from_slice(view.children_of(node));
        if !visit(view, node) {
            return false;
        }
        pending.extend(children.iter().rev());
    }
    true
}

/// Level by level. The first `false` ends the walk. A node's children are
/// queued after its visit, so a visit that compacts a node skips its subtree.
pub(crate) fn walk_breadth_first<V: AstView>(
    view: &mut V,
    from: V::Ref,
    mut visit: impl FnMut(&mut V, V::Ref) -> bool,
) -> bool {
    let mut queue = VecDeque::from([from]);
    while let Some(node) = queue.pop_front() {
        if !visit(view, node) {
            return false;
        }
        queue.extend(view.children_of(node).iter().copied());
    }
    true
}

/// v12's `find`, depth first: the walk stops descending at the first match,
/// but every ancestor of it is still tested, and a matching ancestor takes
/// over. So of a matching node and its matching parent, the parent wins.
pub(crate) fn find<V: AstView>(
    mut view: V,
    from: V::Ref,
    mut predicate: impl FnMut(&V, V::Ref) -> bool,
) -> Option<V::Ref> {
    let mut found = None;
    walk_up(&mut view, from, |view, node| {
        if predicate(view, node) {
            found = Some(node);
            return false;
        }
        true
    });
    found
}

pub(crate) fn find_breadth_first<V: AstView>(
    mut view: V,
    from: V::Ref,
    mut predicate: impl FnMut(&V, V::Ref) -> bool,
) -> Option<V::Ref> {
    let mut found = None;
    walk_breadth_first(&mut view, from, |view, node| {
        if predicate(view, node) {
            found = Some(node);
            return false;
        }
        true
    });
    found
}

/// Every match, children before their parent.
pub(crate) fn find_all<V: AstView>(
    mut view: V,
    from: V::Ref,
    mut predicate: impl FnMut(&V, V::Ref) -> bool,
) -> Vec<V::Ref> {
    let mut found = Vec::new();
    walk_up(&mut view, from, |view, node| {
        if predicate(view, node) {
            found.push(node);
        }
        true
    });
    found
}

pub(crate) fn find_all_breadth_first<V: AstView>(
    mut view: V,
    from: V::Ref,
    mut predicate: impl FnMut(&V, V::Ref) -> bool,
) -> Vec<V::Ref> {
    let mut found = Vec::new();
    walk_breadth_first(&mut view, from, |view, node| {
        if predicate(view, node) {
            found.push(node);
        }
        true
    });
    found
}

/// The leaves, in order (v12 collects them with `walkDown`).
pub(crate) fn flatten<V: AstView>(mut view: V, from: V::Ref) -> Vec<V::Ref> {
    let mut leaves = Vec::new();
    walk_down(&mut view, from, |view, node| {
        if view.children_of(node).is_empty() {
            leaves.push(node);
        }
        true
    });
    leaves
}
