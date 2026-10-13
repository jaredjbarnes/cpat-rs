use std::fmt;

use crate::pattern_error::PatternError;

use super::json::json_string;

/// Why a grammar didn't build. Each variant prints v12's message for the
/// same mistake, character for character, because callers match on it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GrammarError {
    /// The text isn't cpat. `message` is `generate_error_message` for the
    /// cpat grammar's parse of it.
    Syntax { message: String },
    /// `a <|> b`: greedy options were removed in v12. `found` is the whole
    /// expression; line and column are 1-based, the column in UTF-16 units.
    RemovedGreedyOptions {
        found: String,
        line: usize,
        column: usize,
    },
    /// A name that no rule defines, and where it was used: a rule's name, or
    /// `export x` for an export.
    UnknownPattern { name: String, used_in: String },
    /// Rules that only rename each other in a loop, the first name repeated
    /// at the end: `a = b`, `b = a` is `[a, b, a]`.
    AliasCycle { cycle: Vec<String> },
    /// In the choice of rule `rule`, the literal `longer` comes after
    /// `shorter`, which is a proper prefix of it, so it can never match.
    ShadowedLiteral {
        longer: String,
        shorter: String,
        rule: String,
    },
    /// A block delimiter (`["{"]`) with nothing to pair with.
    LoneBlockDelimiter { delimiter: String },
    /// A syntax node the builder has no rule for.
    UnbuildableNode { name: String },
    /// `()` with nothing inside.
    EmptyPatternGroup,
    /// The text nests deeper than a build allows (`grammar/depth.rs`): too
    /// many levels of parentheses, or too long a run of `!`, `?` or
    /// `right`. Line and column are where the level too many begins, 1-based,
    /// the column in UTF-16 units. v12 has no such limit: it overflows the JS
    /// stack (a `RangeError`) or runs out of memory instead.
    NestedTooDeep { line: usize, column: usize },
    /// The grammar imports another, and no way to load it was given.
    NoImportResolver,
    /// The resolver couldn't load `resource`; `message` is its error, which
    /// v12 passes on unchanged.
    ImportResolver { resource: String, message: String },
    /// The grammar imported from `resource` didn't build, or doesn't have
    /// what the import asks of it. Each import wraps the error of the one it
    /// loaded, so a chain of imports nests.
    ImportFailed {
        resource: String,
        details: Box<GrammarError>,
    },
    /// Two imports bring in the same name.
    ImportNameTaken { name: String },
    /// An import names a rule its grammar doesn't define.
    ImportNotFound { name: String, resource: String },
    /// A grammar imports itself, directly or through others: the resources
    /// from the first to import the repeated one, which ends the list too.
    ImportCycle { cycle: Vec<String> },
    /// Imports nest deeper than `MAX_IMPORT_DEPTH` (`grammar/depth.rs`);
    /// `resource` is the import one level too deep.
    ImportsTooDeep { resource: String },
    /// A decorator no one registered; `known` lists the registered names.
    UnknownDecorator {
        decorator: String,
        pattern: String,
        known: Vec<String>,
    },
    /// A decorator refused its pattern; `message` is its error.
    Decorator { message: String },
    /// A decorator's argument is cpat but not JSON (a `\x41` escape, a
    /// leading zero). `message` is the JSON parser's.
    DecoratorArgument { decorator: String, message: String },
    /// A decorator returned a pattern with another name; rules are found by
    /// their pattern's name, so it must keep it.
    DecoratorRenamed {
        decorator: String,
        from: String,
        to: String,
    },
    /// A pattern the grammar describes can't be built (an empty literal, a
    /// regex that doesn't compile).
    Pattern(PatternError),
}

impl fmt::Display for GrammarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GrammarError::Syntax { message } => write!(f, "[Invalid Grammar] {message}"),
            GrammarError::RemovedGreedyOptions {
                found,
                line,
                column,
            } => write!(
                f,
                "[Invalid Grammar] greedy options (`<|>`) were removed in v12; order longer \
                 alternatives first, or use a regex such as /==|=/. Found: `{found}` at line \
                 {line}, column {column}."
            ),
            GrammarError::UnknownPattern { name, used_in } => write!(
                f,
                "[Invalid Grammar] unknown pattern `{name}` used in `{used_in}`; define it in \
                 this grammar, import it, or pass it as a param."
            ),
            GrammarError::AliasCycle { cycle } => {
                let chain: Vec<String> = cycle.iter().map(|name| format!("`{name}`")).collect();
                write!(
                    f,
                    "[Invalid Grammar] alias cycle {}: each rule only renames the next, so none \
                     of them matches anything.",
                    chain.join(" -> ")
                )
            }
            GrammarError::ShadowedLiteral {
                longer,
                shorter,
                rule,
            } => write!(
                f,
                "[Invalid Grammar] `{}` can never match after `{}` in `{rule}`; put the longer \
                 literal first.",
                json_string(longer),
                json_string(shorter)
            ),
            GrammarError::LoneBlockDelimiter { delimiter } => write!(
                f,
                "Block delimiter [{delimiter}] must be used at both the start and end of a \
                 sequence. Example: [\"{{\"] content [\"}}\"]"
            ),
            GrammarError::UnbuildableNode { name } => write!(f, "Couldn't build node: {name}."),
            GrammarError::EmptyPatternGroup => write!(f, "Empty pattern group."),
            GrammarError::NestedTooDeep { line, column } => write!(
                f,
                "[Invalid Grammar] the grammar nests too deeply at line {line}, column \
                 {column}; give the inner expression a rule of its own."
            ),
            GrammarError::NoImportResolver => write!(f, "No import resolver supplied."),
            GrammarError::ImportResolver { message, .. } => write!(f, "{message}"),
            GrammarError::ImportFailed { resource, details } => write!(
                f,
                "Failed loading expression from: \"{resource}\". Error details: \"{details}\""
            ),
            GrammarError::ImportNameTaken { name } => {
                write!(f, "'{name}' was already used within another import.")
            }
            GrammarError::ImportNotFound { name, resource } => write!(
                f,
                "Couldn't find pattern with name: {name}, from import: {resource}."
            ),
            GrammarError::ImportCycle { cycle } => {
                let chain: Vec<String> = cycle.iter().map(|name| format!("`{name}`")).collect();
                write!(
                    f,
                    "[Invalid Grammar] import cycle {}: a grammar can't import itself, even \
                     through others; move what they share into a grammar both import.",
                    chain.join(" -> ")
                )
            }
            GrammarError::ImportsTooDeep { resource } => write!(
                f,
                "[Invalid Grammar] imports nest too deeply at \"{resource}\"; does the resolver \
                 give one grammar a new name each time it is imported?"
            ),
            GrammarError::UnknownDecorator {
                decorator,
                pattern,
                known,
            } => {
                let known: Vec<String> = known.iter().map(|name| format!("@{name}")).collect();
                write!(
                    f,
                    "[Invalid Grammar] unknown decorator `@{decorator}` on `{pattern}`; known \
                     decorators: {}.",
                    known.join(", ")
                )
            }
            GrammarError::Decorator { message } => write!(f, "[Invalid Grammar] {message}"),
            GrammarError::DecoratorArgument { decorator, message } => write!(
                f,
                "[Invalid Grammar] the argument of `@{decorator}` isn't JSON: {message}"
            ),
            GrammarError::DecoratorRenamed {
                decorator,
                from,
                to,
            } => write!(
                f,
                "[Invalid Grammar] `@{decorator}` renamed `{from}` to `{to}`; a decorator must \
                 keep the pattern's name, since other rules find it by that name."
            ),
            GrammarError::Pattern(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for GrammarError {}

impl From<PatternError> for GrammarError {
    fn from(error: PatternError) -> GrammarError {
        GrammarError::Pattern(error)
    }
}
