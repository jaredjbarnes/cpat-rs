//! Grammar files: `.cpat` text in, named patterns out (DESIGN §8).
//!
//! The text is parsed with the hand-written cpat grammar (`cpat.rs`); the
//! builder then walks the read-only tree and makes one pattern per rule.

mod anonymous;
mod block;
mod build;
mod builder;
mod checks;
mod cpat;
mod error;
mod escapes;
mod json;
mod patterns;
mod repeat;
mod syntax;

use std::cell::RefCell;

use crate::error_message::generate_error_message;
use crate::exec::ParseResult;
use crate::pattern::Pattern;
use crate::tree::Node;

pub use cpat::cpat_grammar;
pub use error::GrammarError;
pub use patterns::Patterns;

/// How to build a grammar. Nothing to choose yet: imports, params and
/// decorators arrive with later milestones, and will be set here.
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct GrammarOptions {}

/// Builds patterns from grammar files.
pub struct Grammar;

impl Grammar {
    /// Builds every rule of `text` (v12 `Grammar.parseString`). Each comes
    /// back as a `Context` named after its rule, with every rule in scope,
    /// so a rule can be run on its own.
    ///
    /// Mistakes are errors here, not at first parse: text that isn't cpat,
    /// a name no rule defines, rules that only rename each other in a loop,
    /// and a literal an earlier, shorter one hides.
    pub fn parse_string(text: &str, options: GrammarOptions) -> Result<Patterns, GrammarError> {
        let GrammarOptions {} = options;
        with_cpat_grammar(|grammar| {
            let result = grammar.exec(text);
            let Some(tree) = result.tree() else {
                return Err(syntax_error(grammar, text));
            };
            let root = tree.root();
            reject_removed_syntax(text, root)?;
            builder::build(root)
        })
    }
}

thread_local! {
    // Built once per thread: compiling its regexes on every call would cost
    // more than most grammars take to build. Parsing is single-threaded by
    // design, so a thread-local is the whole story (as for `Name`).
    static CPAT: RefCell<Pattern> = RefCell::new(cpat_grammar());
}

/// Runs `f` with the cpat grammar: the thread's copy, or a fresh one when
/// that is already in use further up the stack.
fn with_cpat_grammar<R>(f: impl FnOnce(&mut Pattern) -> R) -> R {
    CPAT.with(|cell| match cell.try_borrow_mut() {
        Ok(mut grammar) => f(&mut grammar),
        Err(_) => f(&mut cpat_grammar()),
    })
}

/// v12's syntax error: the cpat grammar's error message for `text`, from a
/// recording parse as v12 makes it.
fn syntax_error(grammar: &mut Pattern, text: &str) -> GrammarError {
    let result: ParseResult<'_> = grammar.exec_recording(text);
    GrammarError::Syntax {
        message: generate_error_message(grammar, &result),
    }
}

/// Syntax that was removed from the language is still recognized by the
/// grammar so it can be rejected here with a message that teaches the fix,
/// rather than surfacing as a generic syntax error.
fn reject_removed_syntax(text: &str, root: Node<'_, '_>) -> Result<(), GrammarError> {
    let Some(greedy) =
        syntax::find_outermost(root, |node| node.name() == "removedGreedyOptionsExpr")
    else {
        return Ok(());
    };
    let (line, column) = syntax::line_and_column(text, greedy.start());
    Err(GrammarError::RemovedGreedyOptions {
        found: greedy.value().to_string(),
        line,
        column,
    })
}
