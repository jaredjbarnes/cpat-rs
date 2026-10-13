//! Grammar files: `.cpat` text in, named patterns out (DESIGN §8).
//!
//! The text is parsed with the hand-written cpat grammar (`cpat.rs`); the
//! builder then walks the read-only tree and makes one pattern per rule.

mod anonymous;
mod block;
mod build;
mod builder;
mod camel_case;
mod checks;
mod cpat;
mod decorate;
mod decorators;
mod depth;
mod error;
mod escapes;
mod import;
mod imports;
mod json;
mod options;
mod patterns;
mod repeat;
mod setup;
mod syntax;
mod template;
mod tokens;

use crate::error_message::generate_error_message;
use crate::exec::ParseResult;
use crate::pattern::Pattern;
use crate::tree::Node;

use depth::GRAMMAR_TEXT_LIMITS;
use setup::Setup;

pub use cpat::cpat_grammar;
pub use decorators::{Decorator, Decorators};
pub use error::GrammarError;
pub use imports::{GrammarFile, ImportResolver, InMemoryImports};
pub use options::GrammarOptions;
pub use patterns::Patterns;
pub use template::{patterns, patterns_with};

/// Builds patterns from grammar files.
pub struct Grammar;

impl Grammar {
    /// Builds every rule of `text` (v12 `Grammar.parseString`). Each comes
    /// back as a `Context` named after its rule, with every rule in scope,
    /// so a rule can be run on its own.
    ///
    /// Mistakes are errors here, not at first parse: text that isn't cpat,
    /// a name no rule defines, rules that only rename each other in a loop,
    /// a literal an earlier, shorter one hides, an import that can't be
    /// loaded or doesn't build, and a decorator that refuses its pattern.
    pub fn parse_string(text: &str, options: GrammarOptions) -> Result<Patterns, GrammarError> {
        build_text(text, Setup::top_level(text, options))
    }

    /// Builds the grammar the resolver finds under `resource` (v12
    /// `Grammar.import`), as `parse_string` builds text: its own imports are
    /// resolved from where it was found. The resolver's error, or the
    /// grammar's, comes back as it is.
    pub fn import(resource: &str, options: GrammarOptions) -> Result<Patterns, GrammarError> {
        let Some(resolver) = &options.resolver else {
            return Err(GrammarError::NoImportResolver);
        };
        let file =
            resolver
                .resolve(resource, None)
                .map_err(|message| GrammarError::ImportResolver {
                    resource: resource.to_string(),
                    message,
                })?;
        let options = GrammarOptions {
            origin_resource: Some(file.resource),
            ..options
        };
        Grammar::parse_string(&file.expression, options)
    }

    /// The resources `text`'s import statements name, in statement order:
    /// the first thing its build asks the resolver for (each import then
    /// asks for its own grammar's imports, depth first). A host that loads
    /// grammars asynchronously fetches these ahead of the build and answers
    /// the resolver from memory (DESIGN §19, §22). `None` when the text
    /// wouldn't get as far as its imports (it isn't cpat, or nests too
    /// deeply); its build says why. A `with params` block can't import, so
    /// these are all of the text's imports.
    pub fn imports_of(text: &str) -> Option<Vec<String>> {
        let mut grammar = fresh_cpat_grammar();
        let result = grammar.exec_with(text, GRAMMAR_TEXT_LIMITS);
        let tree = result.tree()?;
        let root = tree.root();
        reject_removed_syntax(text, root).ok()?;
        Some(import::resources(root))
    }
}

/// One grammar's text, built with what `setup` gives it: the caller's own,
/// or one built on behalf of another (an import, a `with params` block).
fn build_text(text: &str, setup: Setup) -> Result<Patterns, GrammarError> {
    let mut grammar = fresh_cpat_grammar();
    let result = grammar.exec_with(text, GRAMMAR_TEXT_LIMITS);
    if let Some(too_deep) = result.too_deep {
        let (line, column) = syntax::line_and_column(text, too_deep.pos);
        return Err(GrammarError::NestedTooDeep { line, column });
    }
    let Some(tree) = result.tree() else {
        return Err(syntax_error(&mut grammar, text));
    };
    let root = tree.root();
    reject_removed_syntax(text, root)?;
    builder::build(text, root, setup)
}

thread_local! {
    // Compiled once per thread: compiling its regexes on every call would
    // cost more than most grammars take to build. Parsing is single-threaded
    // by design, so a thread-local is the whole story (as for `Name`).
    static CPAT: Pattern = cpat_grammar();
}

/// A copy of the cpat grammar that has never parsed. Cheap: the compiled
/// regexes are shared, and references clone their targets only when they
/// first parse. Never the thread's own copy: each reference a parse reaches
/// keeps a clone of what it names, so a grammar that had read a deeply
/// nested text would hold on to all of them for the life of the thread.
fn fresh_cpat_grammar() -> Pattern {
    CPAT.with(Pattern::clone)
}

/// v12's syntax error: the cpat grammar's error message for `text`, from a
/// recording parse as v12 makes it.
fn syntax_error(grammar: &mut Pattern, text: &str) -> GrammarError {
    let result: ParseResult<'_> = grammar.exec_recording_with(text, GRAMMAR_TEXT_LIMITS);
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
