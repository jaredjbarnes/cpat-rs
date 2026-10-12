//! A log of `Literal::parse` calls, for tests that assert which literals a
//! pattern actually tried (the literal-set mode parses only its winner).
//! Compiled only with the `parse-log` feature. The log is per thread, so
//! tests running in parallel can't see each other's parses.

use std::cell::RefCell;

thread_local! {
    static LITERALS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

pub(crate) fn literal(token: &str) {
    LITERALS.with(|log| log.borrow_mut().push(token.to_string()));
}

/// The tokens of the literals parsed on this thread since the last call,
/// in the order they were parsed. Clears the log.
pub fn take_literals() -> Vec<String> {
    LITERALS.with(|log| std::mem::take(&mut *log.borrow_mut()))
}
