//! cpat-wasm: cpat-core for JavaScript.
//!
//! Patterns are built and run in wasm. A parse result comes back as flat
//! typed arrays (see `ExecResult`), which `js/tree.js` wraps in a `Node` view
//! with the familiar read API.

mod exec_result;
mod utf16;

use cpat_core::{Choice, Cut, Literal, Not, Optional, Pattern, Regex, Sequence};
use wasm_bindgen::prelude::*;

pub use exec_result::ExecResult;

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

    pub fn choice(name: &str, alternatives: Vec<JsPattern>) -> Result<JsPattern, JsError> {
        Ok(Choice::new(name, unwrap(alternatives))
            .map_err(js_error)?
            .into())
    }

    pub fn optional(name: &str, child: JsPattern) -> JsPattern {
        Optional::new(name, child.inner).into()
    }

    pub fn not(name: &str, child: JsPattern) -> JsPattern {
        Not::new(name, child.inner).into()
    }

    pub fn cut() -> JsPattern {
        Cut::default().into()
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.inner.name().to_string()
    }

    #[wasm_bindgen(js_name = clone)]
    pub fn clone_pattern(&self) -> JsPattern {
        JsPattern {
            inner: self.inner.clone(),
        }
    }

    pub fn exec(&mut self, text: &str) -> ExecResult {
        let result = self.inner.exec(text);
        ExecResult::from_result(text, &result)
    }

    pub fn test(&mut self, text: &str) -> bool {
        self.inner.test(text)
    }
}

impl<T: Into<Pattern>> From<T> for JsPattern {
    fn from(pattern: T) -> JsPattern {
        JsPattern {
            inner: pattern.into(),
        }
    }
}
