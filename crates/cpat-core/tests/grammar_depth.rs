//! How deeply a grammar's text may nest (`grammar/depth.rs`). Grammar text
//! is input, so too deep a text is a `GrammarError`, never a stack overflow
//! (which aborts the process) or a build whose memory doubles with every
//! level. v12 has no limit: it throws a `RangeError` for a long run of
//! operators, and runs out of memory at about 16 levels of parentheses.

use cpat_core::{Grammar, GrammarError, GrammarOptions};

fn build(text: &str) -> Result<cpat_core::Patterns, GrammarError> {
    Grammar::parse_string(text, GrammarOptions::default())
}

fn nested_too_deep(line: usize, column: usize) -> GrammarError {
    GrammarError::NestedTooDeep { line, column }
}

fn parens(levels: usize) -> String {
    format!("x = {}\"a\"{}", "(".repeat(levels), ")".repeat(levels))
}

#[test]
fn ten_levels_of_parentheses_build() {
    let mut patterns = build(&parens(10)).expect("ten levels are allowed");
    let result = patterns["x"].exec("a");
    assert_eq!(result.tree().map(|tree| tree.root().value()), Some("a"));
}

#[test]
fn eleven_levels_of_parentheses_are_refused_inside_the_eleventh() {
    // `x = ` is 4 columns, so the eleventh `(` is column 15, and what it
    // holds (the level too many) begins at column 16.
    assert_eq!(build(&parens(11)).unwrap_err(), nested_too_deep(1, 16));
}

#[test]
fn a_hundred_levels_of_parentheses_are_refused_too() {
    // Before the limit, each level doubled the build's clones: this never
    // finished. Now it stops at the limit like eleven levels do.
    assert_eq!(build(&parens(100)).unwrap_err(), nested_too_deep(1, 16));
}

#[test]
fn the_limit_counts_nesting_not_parentheses_in_total() {
    // Two groups side by side, each ten deep, are ten levels.
    let group = format!("{}\"a\"{}", "(".repeat(10), ")".repeat(10));
    let text = format!("x = {group} + {group}");
    assert!(build(&text).is_ok());
}

#[test]
fn the_position_is_reported_on_the_right_line() {
    let text = format!("y = \"b\"\n{}", parens(11));
    assert_eq!(build(&text).unwrap_err(), nested_too_deep(2, 16));
}

#[test]
fn the_message_teaches_the_fix() {
    assert_eq!(
        nested_too_deep(3, 7).to_string(),
        "[Invalid Grammar] the grammar nests too deeply at line 3, column 7; give the inner \
         expression a rule of its own."
    );
}

#[test]
fn a_take_until_chain_is_nesting_too() {
    let text = format!("x = {}\"a\"", "?->| ".repeat(20_000));
    assert!(matches!(
        build(&text).unwrap_err(),
        GrammarError::NestedTooDeep { line: 1, .. }
    ));
}

// Prefix and postfix operators nest the built pattern without nesting the
// cpat grammar's parse, so the builder has its own limit: 256 patterns
// inside one another, the innermost operand included.

#[test]
fn two_hundred_fifty_five_nots_build() {
    let text = format!("x = {}\"a\"", "!".repeat(255));
    assert!(build(&text).is_ok());
}

#[test]
fn two_hundred_fifty_six_nots_are_refused_at_the_operand() {
    // The 257th pattern is the literal, after `x = ` and 256 `!`.
    let text = format!("x = {}\"a\"", "!".repeat(256));
    assert_eq!(build(&text).unwrap_err(), nested_too_deep(1, 261));
}

#[test]
fn a_long_run_of_prefix_operators_is_refused_not_a_crash() {
    // 8000 aborted the process with a stack overflow.
    let text = format!("x = {}\"a\"", "!".repeat(8000));
    assert!(matches!(
        build(&text).unwrap_err(),
        GrammarError::NestedTooDeep { line: 1, .. }
    ));
}

#[test]
fn a_long_run_of_optional_operators_is_refused_not_a_crash() {
    let text = format!("x = \"a\"{}", "?".repeat(8000));
    assert!(matches!(
        build(&text).unwrap_err(),
        GrammarError::NestedTooDeep { line: 1, .. }
    ));
}

#[test]
fn a_long_run_of_right_is_refused_not_a_crash() {
    let text = format!("x = \"a\"{}", " right".repeat(20_000));
    assert!(matches!(
        build(&text).unwrap_err(),
        GrammarError::NestedTooDeep { line: 1, .. }
    ));
}
