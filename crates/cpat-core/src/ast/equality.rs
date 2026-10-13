use super::view::AstView;

/// v12's `isEqual`: same type, name, indices and public value, the same
/// number of children, and each child equal in turn. The two nodes may sit
/// in different trees, or different kinds of tree. Pairs are compared from a
/// stack (§7e).
pub(crate) fn is_equal<A: AstView, B: AstView>(
    a: &A,
    a_node: A::Ref,
    b: &B,
    b_node: B::Ref,
) -> bool {
    let mut pending = vec![(a_node, b_node)];
    while let Some((x, y)) = pending.pop() {
        let (xs, ys) = (a.children_of(x), b.children_of(y));
        let same = a.type_of(x) == b.type_of(y)
            && a.name_of(x) == b.name_of(y)
            && a.start_of(x) == b.start_of(y)
            && a.end_of(x) == b.end_of(y)
            && xs.len() == ys.len()
            // v12 compares the public value. A parent's is its children's
            // joined, so once every child pair is equal the parents' values
            // are too: only leaves need comparing, which keeps a deep tree
            // linear rather than a join per level.
            && (!xs.is_empty() || a.own_value_of(x) == b.own_value_of(y));
        if !same {
            return false;
        }
        pending.extend(xs.iter().copied().zip(ys.iter().copied()));
    }
    true
}
