//! How deeply a grammar's text may nest. Grammar text is input, so both
//! limits turn what would be a crash or a runaway into a `GrammarError`.

use crate::halt::Limits;

/// The most composites the cpat grammar may have parsing at once while it
/// reads a grammar's text: a statement takes 8 and each level of `( … )`,
/// `[…] … […]` or `?->|` 3 more, so this allows 10 levels of parentheses.
///
/// Why so few: `patternExpr` tries `( p , divider )+` before `( p )`, so
/// every `(` parses its whole inside twice, and every reference reached
/// keeps its own clone of `patternExpr` (DESIGN §1, §3). Each level of
/// parentheses doubles the clones, as it does in v12. At 10 levels that is
/// about 40 MB for the build; at 14 it was gigabytes, and at 20 the build
/// would not finish. No real grammar nests that deep (the deepest in the
/// fixture corpus is far below it), and a named rule flattens any that did.
pub(super) const GRAMMAR_TEXT_LIMITS: Limits = Limits { max_depth: 38 };

/// The most patterns the builder may be building inside one another. The
/// builder recurses once per level, and prefix and postfix operators
/// (`!!!…"a"`, `"a"???…`, `"a" right right …`) nest without bound in the
/// text without nesting the cpat grammar's parse, so this is the only thing
/// between a long run of them and a stack overflow (a process abort). 256
/// levels is a small fraction of a 1 MB wasm stack even in a debug build,
/// and far deeper than any real rule.
pub(super) const MAX_BUILD_DEPTH: usize = 256;

/// The most grammars that may be importing one another at once. Only a
/// backstop: a cycle is caught by text and origin (`GrammarError::ImportCycle`),
/// but a resolver that answers with a new name or new text on every import
/// would recurse until the stack overflows. Each level holds a builder's state, not the
/// cpat grammar's parse (that has returned before imports are loaded), so
/// 64 is cheap, and far more than any real chain of imports.
pub(super) const MAX_IMPORT_DEPTH: usize = 64;
