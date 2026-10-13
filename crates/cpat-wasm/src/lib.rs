//! cpat-wasm: cpat-core for JavaScript.
//!
//! Patterns are built and run in wasm. A parse result comes back as flat
//! typed arrays (see `ExecResult`), which `js/tree.mjs` wraps in a read-only
//! view and `toNode()` turns into v12's editable `Node`. `js/index.mjs` is
//! the JS API, shaped as clarity-pattern-parser's (DESIGN §22): its pattern
//! classes hold a `Pattern` from here and a path into it (`place.rs`).

mod auto_complete;
mod cursor_json;
mod exec_result;
mod flat_tree;
mod grammar;
mod host;
mod place;
mod repeat_options;
mod run;
mod utf16;

use cpat_core::{
    Block, Choice, Context, Cut, Literal, Not, Optional, Pattern, Reference, Regex, Repeat,
    RepeatOptions, RightAssociated, Sequence, TakeUntil,
};
use wasm_bindgen::prelude::*;

use repeat_options::JsRepeatOptions;

pub use auto_complete::{JsAutoComplete, SuggestionResult};
pub use exec_result::ExecResult;
pub use grammar::GrammarRecord;

/// A pattern living in wasm memory. Composite constructors take ownership of
/// their children, so pass `child.clone()` to keep using a child afterwards.
#[wasm_bindgen(js_name = Pattern)]
pub struct JsPattern {
    inner: Pattern,
}

fn js_error(error: impl std::fmt::Display) -> JsError {
    JsError::new(&error.to_string())
}

fn unwrap(children: Vec<JsPattern>) -> Vec<Pattern> {
    children.into_iter().map(|child| child.inner).collect()
}

/// A block's delimiter. Rust's `Block::new` takes `Literal`s, so the types
/// decide it there; JS hands over any pattern, so it is checked here.
fn delimiter(block: &str, role: &str, pattern: JsPattern) -> Result<Literal, JsError> {
    match pattern.inner {
        Pattern::Literal(literal) => Ok(literal),
        other => Err(JsError::new(&format!(
            "block `{block}`: the {role} delimiter `{}` must be a literal (Pattern.literal), since the block finds its matching close by searching for the tokens",
            other.name()
        ))),
    }
}

#[wasm_bindgen(js_class = Pattern)]
impl JsPattern {
    pub fn literal(name: &str, token: &str) -> Result<JsPattern, JsError> {
        Ok(Literal::new(name, token).map_err(js_error)?.into())
    }

    pub fn regex(name: &str, source: &str) -> Result<JsPattern, JsError> {
        Ok(Regex::new(name, source).map_err(js_error)?.into())
    }

    pub fn sequence(name: &str, children: Vec<JsPattern>) -> Result<JsPattern, JsError> {
        Ok(Sequence::new(name, unwrap(children))
            .map_err(js_error)?
            .into())
    }

    /// `Pattern.choice(name, alternatives)`: the first alternative that
    /// matches wins, unless some alternatives are operators (sequences that
    /// start or end with a reference to `name`), which are assembled by
    /// precedence: declaration order, earlier binds tighter.
    pub fn choice(name: &str, alternatives: Vec<JsPattern>) -> Result<JsPattern, JsError> {
        Ok(Choice::new(name, unwrap(alternatives))
            .map_err(js_error)?
            .into())
    }

    /// `Pattern.repeat(name, child, { min, max, divider, trimDivider })`.
    /// Every option may be left out: one or more, unbounded, no divider. A
    /// `max` of `Infinity` is unbounded, as in clarity-pattern-parser. The
    /// divider, like the child, is taken over by the repeat.
    pub fn repeat(
        name: &str,
        child: JsPattern,
        options: Option<JsRepeatOptions>,
    ) -> Result<JsPattern, JsError> {
        let options = match options {
            Some(options) => repeat_options::read(name, &options)?,
            None => RepeatOptions::default(),
        };
        Ok(Repeat::new(name, child.inner, options).into())
    }

    /// `Pattern.rightAssociated(child)`: marks an infix alternative of a
    /// choice as right-associative (`a ^ b ^ c` is `a ^ (b ^ c)`). Anywhere
    /// else it parses as `child`.
    #[wasm_bindgen(js_name = rightAssociated)]
    pub fn right_associated(child: JsPattern) -> JsPattern {
        RightAssociated::new(child.inner).into()
    }

    pub fn optional(name: &str, child: JsPattern) -> JsPattern {
        Optional::new(name, child.inner).into()
    }

    pub fn not(name: &str, child: JsPattern) -> JsPattern {
        Not::new(name, child.inner).into()
    }

    /// `Pattern.cut(name?)`: v12's `new Cut(name = "cut")`.
    pub fn cut(name: Option<String>) -> JsPattern {
        Cut::new(name.as_deref().unwrap_or("cut")).into()
    }

    /// `Pattern.reference(name, target?)`: the pattern called `target`
    /// (default `name`), found when it first parses: in an enclosing
    /// context, or else in the tree of the pattern being run.
    pub fn reference(name: &str, target: Option<String>) -> JsPattern {
        Reference::to(name, target.as_deref().unwrap_or(name)).into()
    }

    /// `Pattern.context(name, child, patterns)`: parses `child` with
    /// `patterns` in scope for its references. Takes over all of them.
    pub fn context(name: &str, child: JsPattern, patterns: Vec<JsPattern>) -> JsPattern {
        Context::new(name, child.inner, unwrap(patterns)).into()
    }

    /// `Pattern.takeUntil(name, terminator)`: the text up to where
    /// `terminator` matches, or to the end. Taking nothing is a failure.
    #[wasm_bindgen(js_name = takeUntil)]
    pub fn take_until(name: &str, terminator: JsPattern) -> JsPattern {
        TakeUntil::new(name, terminator.inner).into()
    }

    /// `Pattern.block(name, open, content, close)`: `open`, then `content`
    /// (or, when it is `null`, any text) up to the `close` that balances
    /// it. `open` and `close` must be literals; the scan counts their tokens.
    pub fn block(
        name: &str,
        open: JsPattern,
        content: Option<JsPattern>,
        close: JsPattern,
    ) -> Result<JsPattern, JsError> {
        let open = delimiter(name, "open", open)?;
        let close = delimiter(name, "close", close)?;
        Ok(Block::new(name, open, content.map(|content| content.inner), close).into())
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.inner.name().to_string()
    }

    /// Throws when a reference names nothing, with the reason; otherwise
    /// returns nothing. `exec` reports the same through `patternError`.
    pub fn check(&self) -> Result<(), JsError> {
        self.inner.check().map_err(js_error)
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn clone_pattern(&self) -> JsPattern {
        JsPattern {
            inner: self.inner.clone(),
        }
    }

    /// `exec(text, record?)`; see `execAt` for a place below the root.
    pub fn exec(&mut self, text: &str, record: Option<bool>) -> ExecResult {
        let result = if record.unwrap_or(false) {
            self.inner.exec_recording(text)
        } else {
            self.inner.exec(text)
        };
        ExecResult::from_result(text, &result)
    }

    pub fn test(&mut self, text: &str, record: Option<bool>) -> bool {
        if record.unwrap_or(false) {
            self.inner.test_recording(text)
        } else {
            self.inner.test(text)
        }
    }
}

impl<T: Into<Pattern>> From<T> for JsPattern {
    fn from(pattern: T) -> JsPattern {
        JsPattern {
            inner: pattern.into(),
        }
    }
}
