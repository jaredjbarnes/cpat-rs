/// Where the close that balances an open ending at `from` starts, by
/// counting tokens in the text (v12 `Block._scanForMatchingClose`).
///
/// Each close found is checked against the opens between the previous close
/// and it: every open raises the depth, and a close at depth 0 is the match.
/// When the closes run out first (more opens than closes), the last close
/// seen is taken, so a block with stray opens inside still ends at its last
/// close. With no close at all, `None`.
pub(crate) fn matching_close(text: &str, from: usize, open: &str, close: &str) -> Option<usize> {
    let mut from = from;
    let mut depth = 0usize;
    let mut last_close = None;

    loop {
        let Some(close_at) = find(text, close, from) else {
            return last_close;
        };
        last_close = Some(close_at);

        let mut search = from;
        while let Some(open_at) = find(text, open, search).filter(|&at| at < close_at) {
            depth += 1;
            search = open_at + open.len();
        }

        if depth == 0 {
            return Some(close_at);
        }
        depth -= 1;
        from = close_at + close.len();
    }
}

/// `text.indexOf(token, from)`. A `str` search only finds a token where a
/// `char` starts, and every offset passed in is a token's end, so `from` is
/// always a `char` boundary.
fn find(text: &str, token: &str, from: usize) -> Option<usize> {
    text[from..].find(token).map(|at| from + at)
}

#[cfg(test)]
mod tests {
    use super::matching_close;

    #[test]
    fn nested_opens_push_the_match_to_the_balancing_close() {
        assert_eq!(matching_close("{ { } }", 1, "{", "}"), Some(6));
        assert_eq!(matching_close("{ { { } } }", 1, "{", "}"), Some(10));
    }

    #[test]
    fn more_opens_than_closes_takes_the_last_close() {
        assert_eq!(matching_close("{ { { }", 1, "{", "}"), Some(6));
    }

    #[test]
    fn no_close_is_no_match() {
        assert_eq!(matching_close("{ no close", 1, "{", "}"), None);
    }

    #[test]
    fn identical_delimiters_match_the_next_one() {
        // An open found at the close's own position is not before it.
        assert_eq!(matching_close("*a*b*", 1, "*", "*"), Some(2));
    }
}
