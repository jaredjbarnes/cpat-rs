use crate::cursor::Cursor;
use crate::pattern::Pattern;
use crate::tree::NodeId;

use super::already_parsing_here;

/// The ordered mode: the first atom that matches wins, and its node is
/// returned as it is. Runs inside the choice's cut scope. Returns the node,
/// or whether the failure is committed (a cut fired in the alternative that
/// failed).
///
/// Atoms are indices into the alternatives, parsed in place: v12 parses
/// clones, which behave the same (they share ids and start with fresh
/// state), so nothing is cloned here.
pub(super) fn run(
    alternatives: &mut [Pattern],
    atoms: &[usize],
    first: usize,
    cursor: &mut Cursor<'_>,
) -> (Option<NodeId>, bool) {
    for &i in atoms {
        cursor.set_pos(first);
        let alternative = &mut alternatives[i];

        if already_parsing_here(cursor, alternative, first) {
            continue;
        }

        if let Some(node) = alternative.parse(cursor) {
            return (Some(node), false);
        }

        // A cut fired inside the alternative that just failed: the choice is
        // committed to it, so no sibling gets a turn.
        if cursor.cut() {
            return (None, true);
        }

        cursor.resolve_error();
    }

    (None, false)
}
