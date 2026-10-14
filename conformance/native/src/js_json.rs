//! `JSON.stringify` of a decorator argument, so the native engine writes
//! what a recording decorator received in the same text the JS engines do.
//! serde_json's own writer differs from JS in numbers (`1.0`, no `1e+21`),
//! and every JSON number is a double in JS.

use serde_json::Value;

pub fn stringify(value: &Value) -> String {
    let mut out = String::new();
    write(value, &mut out);
    out
}

fn write(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&js_number(n.as_f64().unwrap_or(f64::NAN))),
        // serde_json escapes a string as JSON.stringify does: `"`, `\`, the
        // short escapes and other controls as lowercase `\u00XX`.
        Value::String(s) => out.push_str(&Value::from(s.as_str()).to_string()),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write(item, out);
            }
            out.push(']');
        }
        Value::Object(entries) => {
            out.push('{');
            for (i, (key, item)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                out.push_str(&Value::from(key.as_str()).to_string());
                out.push(':');
                write(item, out);
            }
            out.push('}');
        }
    }
}

/// JS `Number.prototype.toString()` (ECMA-262 Number::toString): the
/// shortest digits that round-trip, written plainly from 1e-7 up to 1e21
/// and with an exponent outside that.
fn js_number(x: f64) -> String {
    if x.is_nan() || x.is_infinite() {
        // JSON.stringify writes a non-finite number as null.
        return "null".to_string();
    }
    if x == 0.0 {
        return "0".to_string();
    }
    let sign = if x < 0.0 { "-" } else { "" };
    // `{:e}` gives the shortest round-trip digits: "d.ddde-7".
    let formatted = format!("{:e}", x.abs());
    let (mantissa, exponent) = formatted.split_once('e').expect("{:e} has an exponent");
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    // The value is 0.digits × 10^n.
    let n = exponent.parse::<i32>().expect("an integer exponent") + 1;
    let body = if k <= n && n <= 21 {
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let e = if e < 0 {
            format!("-{}", -e)
        } else {
            format!("+{e}")
        };
        match k {
            1 => format!("{digits}e{e}"),
            _ => format!("{}.{}e{e}", &digits[..1], &digits[1..]),
        }
    };
    format!("{sign}{body}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn numbers_are_written_as_js_writes_them() {
        // Each expected value is Node 24's `String(x)`.
        let cases = [
            (1.0, "1"),
            (-2.5, "-2.5"),
            (100.0, "100"),
            (1e21, "1e+21"),
            (1.5e21, "1.5e+21"),
            (123456789012345680000.0, "123456789012345680000"),
            (0.000001, "0.000001"),
            (0.0000001, "1e-7"),
            (1.25e-7, "1.25e-7"),
            (0.1 + 0.2, "0.30000000000000004"),
            (-0.0, "0"),
        ];
        for (x, js) in cases {
            assert_eq!(js_number(x), js, "{x}");
        }
    }

    #[test]
    fn values_are_written_without_spaces_in_source_order() {
        let value: Value =
            serde_json::from_str(r#"{"b": [1, 2.0, null, true], "a": "q\"\n"}"#).unwrap();
        assert_eq!(stringify(&value), r#"{"b":[1,2,null,true],"a":"q\"\n"}"#);
        assert_eq!(
            stringify(&json!(12345678901234567890u64)),
            "12345678901234567000"
        );
    }
}
