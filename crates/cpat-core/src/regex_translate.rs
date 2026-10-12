/// Rewrites a JavaScript regex source (as written in `.cpat` files, compiled
/// with the `u` flag) into Rust regex syntax with the same meaning.
///
/// The two dialects agree on most syntax but not all of it, and the
/// differences would silently change what matches:
///
/// - `\d`, `\w`, `\b` are ASCII in JS but Unicode in Rust → spelled out in ASCII.
/// - Inside a class, JS treats `[`, `&`, `~` as literals; Rust treats `[` as a
///   nested class and `&&` / `~~` as set operators → escaped.
/// - JS `[^]` (anything) and `[]` (nothing) have no Rust spelling → rewritten.
/// - `\b` inside a class is a backspace in JS → `\x08`.
/// - `\0` is NUL in JS; Rust has no octal escapes → `\x00`.
/// - `\/` is how `.cpat` escapes its delimiter → `/`.
pub(crate) fn js_to_rust(source: &str) -> String {
    let mut out = String::with_capacity(source.len() + 8);
    let mut chars = source.chars().peekable();
    let mut in_class = false;

    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let Some(escaped) = chars.next() else {
                    out.push('\\');
                    break;
                };
                out.push_str(&translate_escape(escaped, in_class));
            }
            '[' if !in_class => {
                if chars.peek() == Some(&'^') {
                    chars.next();
                    if chars.peek() == Some(&']') {
                        chars.next();
                        out.push_str("(?s:.)");
                        continue;
                    }
                    out.push_str("[^");
                } else if chars.peek() == Some(&']') {
                    chars.next();
                    out.push_str("[^\\x00-\\x{10FFFF}]");
                    continue;
                } else {
                    out.push('[');
                }
                in_class = true;
            }
            ']' if in_class => {
                in_class = false;
                out.push(']');
            }
            '[' | '&' | '~' if in_class => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }

    out
}

fn translate_escape(escaped: char, in_class: bool) -> String {
    const DIGIT: &str = "0-9";
    const WORD: &str = "0-9A-Za-z_";

    match (escaped, in_class) {
        ('d', false) => format!("[{DIGIT}]"),
        ('d', true) => DIGIT.to_string(),
        ('w', false) => format!("[{WORD}]"),
        ('w', true) => WORD.to_string(),
        // Negated classes nest inside an enclosing class in Rust syntax.
        ('D', _) => format!("[^{DIGIT}]"),
        ('W', _) => format!("[^{WORD}]"),
        ('b', false) => "(?-u:\\b)".to_string(),
        ('B', false) => "(?-u:\\B)".to_string(),
        ('b', true) => "\\x08".to_string(),
        ('0', _) => "\\x00".to_string(),
        ('/', _) => "/".to_string(),
        (other, _) => format!("\\{other}"),
    }
}

/// Whether the source ends with an unescaped `$`.
pub(crate) fn ends_with_anchor(source: &str) -> bool {
    let Some(before) = source.strip_suffix('$') else {
        return false;
    };
    let backslashes = before.chars().rev().take_while(|&c| c == '\\').count();
    backslashes % 2 == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spells_ascii_classes_out() {
        assert_eq!(js_to_rust(r"\d+"), "[0-9]+");
        assert_eq!(js_to_rust(r"[\d_]"), "[0-9_]");
        assert_eq!(js_to_rust(r"\w"), "[0-9A-Za-z_]");
        assert_eq!(js_to_rust(r"[^\W]"), "[^[^0-9A-Za-z_]]");
        assert_eq!(js_to_rust(r"\bx"), r"(?-u:\b)x");
    }

    #[test]
    fn escapes_what_rust_reads_as_class_syntax() {
        assert_eq!(js_to_rust(r"[a&&b]"), r"[a\&\&b]");
        assert_eq!(js_to_rust(r"[[]"), r"[\[]");
        assert_eq!(js_to_rust(r"[~]"), r"[\~]");
    }

    #[test]
    fn rewrites_js_only_forms() {
        assert_eq!(js_to_rust(r"[^]"), "(?s:.)");
        assert_eq!(js_to_rust(r"[]"), r"[^\x00-\x{10FFFF}]");
        assert_eq!(js_to_rust(r"[\b]"), r"[\x08]");
        assert_eq!(js_to_rust(r"\0"), r"\x00");
        assert_eq!(js_to_rust(r"a\/b"), "a/b");
    }

    #[test]
    fn leaves_shared_syntax_alone() {
        assert_eq!(js_to_rust(r"(?:a|b)*\s\.[^\n]"), r"(?:a|b)*\s\.[^\n]");
        assert_eq!(js_to_rust(r"over(?![a-z-])"), r"over(?![a-z-])");
    }

    #[test]
    fn finds_only_unescaped_end_anchors() {
        assert!(ends_with_anchor(".$"));
        assert!(ends_with_anchor(r"\\$"));
        assert!(!ends_with_anchor(r"\$"));
        assert!(!ends_with_anchor("a"));
    }
}
