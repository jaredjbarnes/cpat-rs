//! How long a path a query follows (milestone 8, fix round 2). A path is a
//! public argument, so a caller can make one far longer than any parse
//! would. Each step adds a link to the query's chain, which the climb in
//! `next_tokens` recurses up and the drop recurses down: a 300 000-step path
//! aborted the process with a stack overflow. A query now follows a path
//! only as long as a parse under the same `Limits` can make, and treats a
//! longer one as leading nowhere. v12 has no such limit (its paths are
//! parent pointers, made by real parses); a real path is never refused.

mod common;

use common::*;
use cpat_core::{
    DEFAULT_MAX_DEPTH, Introspection, Limits, Pattern, PatternPath, generate_error_message,
};

/// `expr = "(" expr close | "x"`.
fn parens(close: Pattern) -> Pattern {
    choice(
        "expr",
        vec![
            seq("group", vec![lit("open", "("), reference("expr"), close]),
            lit("x", "x"),
        ],
    )
}

/// The path of the `x` under `levels` parentheses: `/0/1/0` per level
/// (group, its reference, the reference's target), then `/1` for the `x`.
fn leaf_under(levels: usize) -> Vec<u32> {
    let mut steps: Vec<u32> = [0, 1, 0].repeat(levels);
    steps.push(1);
    steps
}

/// Runs `run` on a thread with `cargo test`'s and wasm's small stack.
fn on_small_stack<T: Send + 'static>(run: impl FnOnce() -> T + Send + 'static) -> T {
    on_stack(2 << 20, run)
}

fn on_stack<T: Send + 'static>(bytes: usize, run: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(bytes)
        .spawn(run)
        .unwrap()
        .join()
        .unwrap()
}

#[test]
fn the_parse_and_the_path_agree_on_the_leaf() {
    // The paths below are built by hand; this pins that they are the ones
    // a parse makes.
    let mut expr = parens(lit("close", ")"));
    let result = expr.exec("(((x");
    assert_eq!(
        result.leaf_match.unwrap().path,
        PatternPath::new(&leaf_under(3))
    );
}

#[test]
fn a_path_far_longer_than_any_parse_makes_leads_nowhere() {
    // The verifier's probe: 100 000 levels, 300 001 steps. Before the
    // bound this aborted with "thread has overflowed its stack" (a debug
    // build overflowed the 8 MB main thread from about 30 000 levels).
    let answers = on_small_stack(|| {
        let expr = parens(lit("close", ")"));
        let path = PatternPath::new(&leaf_under(100_000));
        let introspection = Introspection::new(&expr);
        (
            introspection.at(&path).is_none(),
            introspection.tokens(&path),
            introspection.patterns(&path).len(),
            introspection.next_patterns(&path).len(),
            expr.next_tokens(&path),
        )
    });

    assert_eq!(answers, (true, vec![], 0, 0, vec![]));
}

#[test]
fn a_path_is_followed_up_to_the_parse_depth_limit_and_no_further() {
    // A parse under the default limits makes paths of at most
    // `DEFAULT_MAX_DEPTH` steps (every step below the root enters a
    // composite, which takes a frame, or ends at a leaf).
    let expr = parens(lit("close", ")"));
    let steps = leaf_under(400);
    let introspection = Introspection::new(&expr);

    let longest = PatternPath::new(&steps[..DEFAULT_MAX_DEPTH]);
    let too_long = PatternPath::new(&steps[..DEFAULT_MAX_DEPTH + 1]);

    assert!(introspection.at(&longest).is_some());
    assert!(introspection.at(&too_long).is_none());
}

#[test]
fn the_longest_path_followed_climbs_to_the_root_on_a_small_stack() {
    // With the close optional, what follows the `x` climbs every level to
    // the root, one `)` per level: the deepest climb a caller can ask for
    // under the default limits (341 levels, 1024 steps, one level past the
    // deepest a parse reaches). It must fit the 2 MB stack.
    let tokens = on_small_stack(|| {
        let expr = parens(opt("opt-close", lit("close", ")")));
        let steps = leaf_under(341);
        assert_eq!(steps.len(), DEFAULT_MAX_DEPTH);
        expr.next_tokens(&PatternPath::new(&steps))
    });

    assert_eq!(tokens, vec![")"; 341]);
}

#[test]
fn raised_limits_follow_a_longer_path() {
    // `exec_with` makes deeper paths under raised limits; introspection
    // under the same limits follows them.
    let expr = parens(lit("close", ")"));
    let path = PatternPath::new(&leaf_under(400));
    let raised = Limits { max_depth: 2048 };

    assert!(Introspection::new(&expr).at(&path).is_none());
    let leaf = Introspection::with_limits(&expr, raised).at(&path);
    assert_eq!(leaf.map(|leaf| leaf.name()), Some("x"));
}

#[test]
fn the_error_message_follows_the_limits_the_parse_ran_under() {
    // A parse under raised limits puts the leaf 1201 steps down, past the
    // default limit; the hint must still climb from it. Run on the stack
    // the raised limits ask for. v12 printed the same 400 `)`, column 402
    // (Expression `expr` with an optional close, input "(" x 400 + "x[").
    let (path_len, message) = on_stack(16 << 20, || {
        let mut expr = parens(opt("opt-close", lit("close", ")")));
        let text = format!("{}x[", "(".repeat(400));
        let result = expr.exec_with(&text, Limits { max_depth: 2048 });
        assert!(result.too_deep.is_none());
        assert_eq!(result.limits(), Limits { max_depth: 2048 });
        let path_len = result.leaf_match.as_ref().unwrap().path.steps().len();
        (path_len, generate_error_message(&expr, &result))
    });

    assert_eq!(path_len, 1201);
    let hint = vec![")"; 400].join(",");
    assert_eq!(
        message,
        format!("Error at line 1, column 402. Hint: {hint}")
    );
}
