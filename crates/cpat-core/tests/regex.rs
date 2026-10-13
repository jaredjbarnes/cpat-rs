mod common;

use common::*;
use cpat_core::{Cursor, NodeKind, Pattern, PatternError, Regex, Tokenizable};

#[test]
fn empty_source_is_rejected() {
    assert!(matches!(
        Regex::new("empty", ""),
        Err(PatternError::EmptyRegex { .. })
    ));
}

#[test]
fn anchors_are_rejected() {
    assert!(matches!(
        Regex::new("carrot", "^"),
        Err(PatternError::AnchoredRegex { .. })
    ));
    assert!(matches!(
        Regex::new("money", ".$"),
        Err(PatternError::AnchoredRegex { .. })
    ));
}

#[test]
fn an_escaped_dollar_is_not_an_anchor() {
    let mut price = re("price", r"\$[0-9]+");
    assert!(price.test("$42"));
}

#[test]
fn an_invalid_source_is_rejected_with_the_reason() {
    let error = Regex::new("broken", "(").unwrap_err();
    assert!(matches!(error, PatternError::InvalidRegex { .. }));
    assert!(error.to_string().contains("broken"), "{error}");
}

#[test]
fn successful_parse() {
    let mut number = re("number", r"\d");
    let mut cursor = Cursor::new("1");

    let id = number.parse(&mut cursor).unwrap();

    assert!(!cursor.has_error());
    let tree = cursor.into_tree(id);
    let node = tree.root();
    assert_eq!(node.kind(), NodeKind::Regex);
    assert_eq!(
        (node.name(), node.start(), node.end(), node.value()),
        ("number", 0, 1, "1")
    );
}

#[test]
fn failed_parse() {
    let mut number = re("number", r"\d");
    let mut cursor = Cursor::new("F");

    assert!(number.parse(&mut cursor).is_none());

    let error = cursor.error().unwrap();
    assert_eq!((error.start, error.last), (0, 0));
}

#[test]
fn exec_and_test() {
    let mut regex = re("a", "A");

    assert!(regex.exec("B").tree.is_none());
    assert!(regex.test("A"));
    assert!(!regex.test("B"));
}

#[test]
fn source_is_kept_as_written() {
    let regex = Regex::new("number", r"\d+").unwrap();
    assert_eq!(regex.source(), r"\d+");
}

#[test]
fn node_value_covers_the_grapheme_snapped_span() {
    // "e" + combining acute is one grapheme. The match ends inside it, so the
    // node takes the whole grapheme, and its value is the text it spans.
    let mut letter = re("letter", "e");
    let mut cursor = Cursor::new("e\u{301}x");

    let id = letter.parse(&mut cursor).unwrap();

    let tree = cursor.into_tree(id);
    let node = tree.root();
    assert_eq!((node.start(), node.end()), (0, 3));
    assert_eq!(node.value(), "e\u{301}");
}

#[test]
fn matches_at_the_cursor_not_anywhere_after_it() {
    let mut digits_after_a = seq("s", vec![lit("a", "a"), re("digits", "[0-9]+")]);

    assert!(digits_after_a.test("a12"));
    assert!(!digits_after_a.test("ax12"));
}

#[test]
fn ascii_classes_keep_their_javascript_meaning() {
    // `\d` is ASCII in JavaScript; Rust's would also match other scripts' digits.
    let mut digit = re("digit", r"\d");
    assert!(digit.test("7"));
    assert!(!digit.test("٣"), "ARABIC-INDIC DIGIT THREE is not a JS \\d");

    let mut word = re("word", r"\w+");
    assert!(word.test("abc_1"));
    assert!(!word.test("é"));
}

#[test]
fn word_boundaries_see_the_cursor_as_the_start_of_input() {
    // The regex runs on the text from the cursor on, as the JS library's
    // substring does, so `\b` holds at the cursor even after a word character.
    let mut pattern = seq("s", vec![lit("a", "a"), re("x", r"\bx")]);
    assert!(pattern.test("ax"));
}

#[test]
fn lookaround_uses_the_backtracking_engine() {
    let keyword = Regex::new("over", "over(?![a-z-])").unwrap();
    assert!(keyword.is_fancy());

    let mut clause = seq("clause", vec![keyword.into(), re("rest", ".+")]);
    assert!(clause.test("over 5"));
    assert!(!clause.test("overly"));
}

#[test]
fn a_backtracking_regex_matches_a_long_run_as_js_does() {
    // The shell grammar's `dqLiteral`, on the 500 KB string an agent-harness
    // test runs through it. fancy-regex's default backtrack limit gave up at
    // about 500 000 characters; JS matches the whole run.
    let source = r#"(?:\\[\s\S]|\$(?![{(A-Za-z_?@*#0-9])|[^"\\$`])+"#;
    let literal = Regex::new("dqLiteral", source).unwrap();
    assert!(literal.is_fancy());
    let run = "a".repeat(600_000);
    let mut literal: Pattern = literal.into();
    assert_eq!(
        parse_value(&mut literal, &format!("{run}\"")).map(|v| v.len()),
        Some(600_000)
    );
}

#[test]
fn plain_sources_use_the_linear_engine() {
    assert!(!Regex::new("ws", r"[ \t]+").unwrap().is_fancy());
}

#[test]
fn an_escaped_slash_matches_a_slash() {
    let mut path = re("path", r"a\/b");
    assert!(path.test("a/b"));
}

#[test]
fn tokens_can_be_set() {
    let mut regex = Regex::new("a", "A").unwrap();
    regex.set_tokens(vec!["A".into()]);
    assert_eq!(regex.tokens(), ["A"]);
}

#[test]
fn word_boundaries_stay_ascii_in_the_backtracking_engine() {
    // jq's grammar writes keywords as `(?!(?:if|…)\b)[a-zA-Z_]…`. A
    // lookahead needs `fancy-regex`, which refused the linear engine's
    // spelling of an ASCII `\b`, so the grammar didn't build. Expected
    // values are JS's (`u` flag): `é` is not a word character there.
    let identifier = Regex::new("identifier", r"(?!if\b)[a-z]+").unwrap();
    assert!(identifier.is_fancy());
    let mut identifier: Pattern = identifier.into();
    assert!(identifier.test("iffy"));
    assert!(!identifier.test("if"));

    let mut boundary = re("boundary", r"(?=a)a\b");
    assert_eq!(parse_value(&mut boundary, "aé"), Some("a".to_string()));
    let mut no_boundary = re("no-boundary", r"(?=a)a\B");
    assert_eq!(parse_value(&mut no_boundary, "ab"), Some("a".to_string()));
    assert_eq!(parse_value(&mut no_boundary, "aé"), None);
}

#[test]
fn the_dot_stops_at_every_js_line_terminator() {
    // JS's `.` (u flag) refuses \n, \r, U+2028 and U+2029; Rust's refuses
    // only \n, so all three of these used to match.
    let mut dot = re("dot", "a.b");
    for text in ["a\rb", "a\nb", "a\u{2028}b", "a\u{2029}b"] {
        assert!(!dot.test(text), "{text:?}");
    }
    assert!(dot.test("a b"));
    assert!(dot.test("a\u{85}b"));
}

#[test]
fn whitespace_is_js_whitespace() {
    // JS's `\s` has U+FEFF and lacks U+0085; Unicode's White_Space (Rust's
    // `\s`) is the other way round.
    let mut space = re("space", r"a\sb");
    assert!(space.test("a\u{FEFF}b"));
    assert!(!space.test("a\u{85}b"));

    let mut not_space = re("not-space", r"a\Sb");
    assert!(!not_space.test("a\u{FEFF}b"));
    assert!(not_space.test("a\u{85}b"));

    let mut in_class = re("in-class", r"[\s]+");
    assert_eq!(
        parse_value(&mut in_class, "\t\u{B}\u{3000}\u{FEFF}\u{85}"),
        Some("\t\u{B}\u{3000}\u{FEFF}".to_string())
    );
}

#[test]
fn control_escapes_name_control_characters() {
    let mut newline = re("newline", r"a\cJb");
    assert!(newline.test("a\nb"));
    assert!(!newline.test("acJb"));
}

#[test]
fn a_set_as_a_class_range_end_is_a_build_error_as_in_js() {
    // JS's `u` flag rejects `[\d-z]`; Rust would read it as digits, `-`, `z`.
    for source in [
        r"[\d-z]",
        r"\p{letter}",
        r"a{,3}",
        r"\-",
        r"\x{41}",
        r"(?=a)*",
    ] {
        let error = Regex::new("r", source).unwrap_err();
        assert!(
            matches!(error, PatternError::InvalidRegex { .. }),
            "{source}: {error:?}"
        );
    }
    let error = Regex::new("r", r"[\d-z]").unwrap_err();
    assert!(
        error.to_string().contains("Invalid character class"),
        "{error}"
    );
}

#[test]
fn a_backreference_to_a_group_that_did_not_match_reads_as_empty() {
    // JS reads an unset group as empty; fancy-regex alone would fail.
    let mut optional = re("optional", r"(a)?b\1");
    assert_eq!(parse_value(&mut optional, "b"), Some("b".to_string()));
    assert_eq!(parse_value(&mut optional, "aba"), Some("aba".to_string()));
    let mut forward = re("forward", r"\1(a)");
    assert_eq!(parse_value(&mut forward, "a"), Some("a".to_string()));
}

#[test]
fn a_backreference_a_repetition_resets_is_refused() {
    // JS clears group 1 on every round, so `abb` matches there; fancy-regex
    // keeps the first round's `a`. Refused rather than matched differently.
    let error = Regex::new("r", r"(?:(a)|b)+\1").unwrap_err();
    assert!(
        error.to_string().contains("can't match it the same way"),
        "{error}"
    );
}

#[test]
fn modifier_groups_keep_their_js_meaning() {
    let mut multiline = re("multiline", r"(?m:a$)");
    assert_eq!(parse_value(&mut multiline, "a\rb"), Some("a".to_string()));
    let mut dot_all = re("dot-all", r"(?s:a.b)");
    assert!(dot_all.test("a\u{2028}b"));
    let mut folded = re("folded", r"(?i:\w)");
    assert!(folded.test("\u{17F}"), "JS's \\w under i takes the long s");
}

/// JS complements `\P{…}` before folding it under `i`; Rust's `(?i)` folds
/// first. v12's answers, recorded under tsx.
#[test]
fn a_negated_property_under_i_complements_before_folding() {
    let rows: &[(&str, &str, Option<&str>)] = &[
        (r"(?i:\P{Lu})", "A", Some("A")),
        (r"(?i:\P{Ll})", "a", Some("a")),
        (r"(?i:\P{Lowercase})", "a", Some("a")),
        (r"(?i:\P{ASCII})", "k", Some("k")),
        (r"(?i:\P{Lt})", "\u{1C6}", Some("\u{1C6}")),
        (r"(?i:[\P{Lu}x])", "A", Some("A")),
        (r"(?i:[^\P{Lu}])", "a", None),
        (r"(?i:[^\P{Lu}])", "A", None),
        (r"(?i:\P{ASCII})", "\u{E9}", Some("\u{E9}")),
        (r"(?i:\P{Any})", "a", None),
        (r"(?i:[\P{Any}])", "a", None),
        (r"(?i:[^\P{Any}])", "a", Some("a")),
        (r"(?i:\P{Lu}(?<=a))", "a", Some("a")),
        (r"(?i:(?-i:\P{Lu}))", "A", None),
        (r"\P{Lu}", "A", None),
        (r"(?i:\P{Lu})", "1", Some("1")),
        (r"(?i:\P{Script=Latin})", "a", None),
        (r"(?i:\P{Script=Greek})", "\u{2126}", None),
        (r"(?i:\P{Ll}+)", "aBc1", Some("aBc1")),
        (r"(?i:[^\P{Ll}])", "A", None),
        (r"(?i:[\P{Lu}\P{Ll}])", "A", Some("A")),
        (r"(?i:\P{Lu})", "\u{212A}", Some("\u{212A}")),
        (r"(?i:\P{Ll})", "\u{212A}", Some("\u{212A}")),
    ];
    for &(source, text, expected) in rows {
        let mut pattern = re("r", source);
        assert_eq!(
            parse_value(&mut pattern, text).as_deref(),
            expected,
            "{source} on {text:?}"
        );
    }
}

/// The value `pattern` matches at the start of `text`, if any.
fn parse_value(pattern: &mut Pattern, text: &str) -> Option<String> {
    let mut cursor = Cursor::new(text);
    let node = pattern.parse(&mut cursor)?;
    Some(text[cursor.node_start(node)..cursor.node_end(node)].to_string())
}
