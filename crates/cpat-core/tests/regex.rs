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

/// The value `pattern` matches at the start of `text`, if any.
fn parse_value(pattern: &mut Pattern, text: &str) -> Option<String> {
    let mut cursor = Cursor::new(text);
    let node = pattern.parse(&mut cursor)?;
    Some(text[cursor.node_start(node)..cursor.node_end(node)].to_string())
}
