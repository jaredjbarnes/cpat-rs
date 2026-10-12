//! The literal-set mode (v12 `LiteralSet`): an ordered choice of literals
//! decided with a first-character index instead of trying each literal.
//!
//! It reproduces what the ordered loop would leave behind: which literal
//! wins (the first in declaration order), and which failure would end up
//! the furthest error (the FIRST failed literal with the greatest `last`,
//! because the cursor replaces the furthest error only on a strictly
//! greater `last`). Recording that one failure leaves the same furthest
//! error the loop's many records would; nothing else a failed literal does
//! survives the choice, which resets the cursor either way.
//!
//! **Why the key is the first `char`:** `Literal` compares `char` by `char`
//! (`literal::mismatch`), so a literal whose first `char` differs from the
//! text's fails on its first comparison, at the start index, which is the
//! least `last` any failure can have. Only the bucket that shares the
//! text's first `char` can win or fail further, and those literals are
//! compared with `mismatch` itself, so the rule can't drift from
//! `Literal::parse`. (A first-byte key would be correct too, since a
//! differing first byte means a differing first `char`, but its buckets
//! would hold literals that only share a UTF-8 lead byte, `é` and `è`, and
//! still fail at the start.)

use crate::ancestors::At;
use crate::cursor::Cursor;
use crate::pattern::Pattern;
use crate::pattern_id::Meta;
use crate::pattern_path::step;
use crate::patterns::literal::mismatch;
use crate::tree::NodeId;

/// One literal alternative: its token, and the pattern its failure is
/// recorded under (for a reference, the target renamed after the
/// reference, as the reference's instance would record it).
#[derive(Clone, Debug)]
pub(super) struct Entry {
    pub(super) token: Box<str>,
    pub(super) meta: Meta,
}

#[derive(Clone, Debug)]
pub(crate) struct LiteralSet {
    /// Indices of the alternatives, in declaration order; `entries[k]` is
    /// what `alternatives[atoms[k]]` matches.
    pub(super) atoms: Vec<usize>,
    entries: Vec<Entry>,
    /// Positions in `entries`, bucketed by the token's first `char`, each
    /// bucket in declaration order, sorted by key for a binary search.
    buckets: Vec<(char, Vec<usize>)>,
}

/// What the ordered loop would observe at one index.
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Outcome {
    /// The first entry (in order) that matches.
    pub(super) winner: Option<usize>,
    /// The failure among the entries tried before the winner (all of them
    /// when none wins) that the loop would leave as the furthest error:
    /// `(entry, last)`.
    pub(super) furthest: Option<(usize, usize)>,
}

impl LiteralSet {
    pub(super) fn new(atoms: Vec<usize>, entries: Vec<Entry>) -> LiteralSet {
        debug_assert_eq!(atoms.len(), entries.len());
        let mut buckets: Vec<(char, Vec<usize>)> = Vec::new();
        for (k, entry) in entries.iter().enumerate() {
            let key = first_char(&entry.token);
            match buckets.iter_mut().find(|(c, _)| *c == key) {
                Some((_, bucket)) => bucket.push(k),
                None => buckets.push((key, vec![k])),
            }
        }
        buckets.sort_by_key(|(key, _)| *key);
        LiteralSet {
            atoms,
            entries,
            buckets,
        }
    }

    /// What trying every entry in order at `first` would do. `first` must
    /// be inside the text: at the end every literal fails without comparing
    /// anything, and the caller runs the ordered loop there (v12 does too).
    pub(super) fn outcome(&self, text: &str, first: usize) -> Outcome {
        let key = text[first..]
            .chars()
            .next()
            .expect("a literal set runs only inside the text");
        let bucket = match self.buckets.binary_search_by_key(&key, |(c, _)| *c) {
            Ok(found) => &self.buckets[found].1[..],
            Err(_) => &[],
        };

        let mut winner = None;
        let mut furthest: Option<(usize, usize)> = None;
        for &k in bucket {
            match mismatch(text, first, &self.entries[k].token) {
                None => {
                    winner = Some(k);
                    break;
                }
                Some(last) => {
                    if furthest.is_none_or(|(_, best)| last > best) {
                        furthest = Some((k, last));
                    }
                }
            }
        }

        // Every literal outside the bucket fails right at `first`. If no
        // bucket literal got further, the loop's furthest failure is simply
        // the first literal it tried.
        let tried = winner.unwrap_or(self.entries.len());
        if tried > 0 && furthest.is_none_or(|(_, best)| best <= first) {
            furthest = Some((0, first));
        }

        Outcome { winner, furthest }
    }

    /// The literal-set parse (v12 `_parseLiteralChoice`): record the failure
    /// the loop would leave furthest, then parse only the winner, in place,
    /// as the loop would have parsed it (a reference alternative still
    /// pushes its frame and names the node).
    ///
    /// v12 resolves the recorded error straight away. Here the choice does
    /// that on success (and the winner's own success already has), and on
    /// failure records its own error over it, so a resolve in between
    /// could never be seen.
    pub(super) fn run(
        &self,
        alternatives: &mut [Pattern],
        first: usize,
        cursor: &mut Cursor<'_>,
    ) -> Option<NodeId> {
        let outcome = self.outcome(cursor.text(), first);

        if let Some((k, last)) = outcome.furthest {
            // Recorded under the atom. A recording parse never gets here
            // (it runs the loop, `literal_set_applies`), so no path is ever
            // built from this, and a reference's instance one step further
            // down needs no telling apart.
            cursor.record_error_at(first, last, self.entries[k].meta, At::Below(&[step(k)]));
        }

        let winner = outcome.winner?;
        cursor.set_child(step(winner));
        alternatives[self.atoms[winner]].parse(cursor)
    }
}

fn first_char(token: &str) -> char {
    token
        .chars()
        .next()
        .expect("a literal's token is never empty")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(tokens: &[&str]) -> LiteralSet {
        let entries = tokens
            .iter()
            .map(|token| Entry {
                token: (*token).into(),
                meta: Meta::new(token),
            })
            .collect();
        LiteralSet::new((0..tokens.len()).collect(), entries)
    }

    fn outcome(tokens: &[&str], text: &str) -> Outcome {
        set(tokens).outcome(text, 0)
    }

    #[test]
    fn the_first_matching_literal_wins_even_when_a_later_one_is_longer() {
        let found = outcome(&["i", "if", "in"], "if");
        assert_eq!(found.winner, Some(0));
        assert_eq!(found.furthest, None, "nothing was tried before the winner");
    }

    #[test]
    fn the_furthest_failure_is_the_first_with_the_greatest_last() {
        // "ab" and "ac" both fail at 1 on "ax"; the loop keeps the first.
        let found = outcome(&["z", "ab", "ac"], "ax");
        assert_eq!(found.winner, None);
        assert_eq!(found.furthest, Some((1, 1)));
    }

    #[test]
    fn with_no_bucket_literal_past_the_start_the_first_literal_is_furthest() {
        // "b" (outside the bucket) and "ab" (running out of text at 0)
        // both fail at 0: the first literal tried is the one kept.
        assert_eq!(outcome(&["b", "ab"], "a").furthest, Some((0, 0)));
        assert_eq!(outcome(&["b", "c"], "z").furthest, Some((0, 0)));
    }

    #[test]
    fn literals_after_the_winner_are_never_tried() {
        // "abc" would fail further (at 2), but comes after the winner.
        let found = outcome(&["x", "ab", "abc"], "abd");
        assert_eq!(found.winner, Some(1));
        assert_eq!(found.furthest, Some((0, 0)));
    }

    #[test]
    fn the_winner_first_in_order_has_nothing_before_it() {
        assert_eq!(outcome(&["a", "b"], "a").furthest, None);
    }

    #[test]
    fn literals_sharing_only_a_utf8_lead_byte_fail_at_the_start() {
        // é is C3 A9, è is C3 A8: different chars, so different buckets.
        let found = outcome(&["è", "éx"], "éy");
        assert_eq!(found.winner, None);
        // "éx" compares é, then fails on y at byte 2.
        assert_eq!(found.furthest, Some((1, 2)));
    }
}
