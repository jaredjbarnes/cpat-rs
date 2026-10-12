use cpat_core::RepeatOptions;
use wasm_bindgen::convert::TryFromJsValue;
use wasm_bindgen::prelude::*;

use crate::JsPattern;

#[wasm_bindgen]
extern "C" {
    /// The plain object `Pattern.repeat` takes: `{ min, max, divider,
    /// trimDivider }`, every field optional. Read through getters so JS
    /// passes an ordinary object literal, not a wasm-built class.
    #[wasm_bindgen(
        typescript_type = "{ min?: number; max?: number; divider?: Pattern; trimDivider?: boolean }"
    )]
    pub type JsRepeatOptions;

    #[wasm_bindgen(method, getter)]
    fn min(this: &JsRepeatOptions) -> Option<f64>;

    #[wasm_bindgen(method, getter)]
    fn max(this: &JsRepeatOptions) -> Option<f64>;

    #[wasm_bindgen(method, getter)]
    fn divider(this: &JsRepeatOptions) -> JsValue;

    #[wasm_bindgen(method, getter, js_name = trimDivider)]
    fn trim_divider(this: &JsRepeatOptions) -> Option<bool>;
}

/// Converts and validates the JS options. A count must be a whole number of
/// zero or more; `max` may also be `Infinity`, meaning no bound.
pub(crate) fn read(name: &str, options: &JsRepeatOptions) -> Result<RepeatOptions, JsError> {
    let defaults = RepeatOptions::default();

    let min = match options.min() {
        None => defaults.min,
        Some(min) => count(name, "min", min)?,
    };
    let max = match options.max() {
        None => None,
        Some(max) if max == f64::INFINITY => None,
        Some(max) => Some(count(name, "max", max)?),
    };
    let divider = options.divider();
    let divider = if divider.is_undefined() || divider.is_null() {
        None
    } else {
        let pattern = JsPattern::try_from_js_value(divider)
            .map_err(|_| JsError::new(&format!("repeat `{name}`: divider must be a Pattern")))?;
        Some(pattern.inner)
    };

    Ok(RepeatOptions {
        min,
        max,
        divider,
        trim_divider: options.trim_divider().unwrap_or(false),
    })
}

fn count(name: &str, field: &str, value: f64) -> Result<usize, JsError> {
    if value >= 0.0 && value.fract() == 0.0 && value <= u32::MAX as f64 {
        return Ok(value as usize);
    }
    Err(JsError::new(&format!(
        "repeat `{name}`: {field} must be a whole number of zero or more, got {value}"
    )))
}
