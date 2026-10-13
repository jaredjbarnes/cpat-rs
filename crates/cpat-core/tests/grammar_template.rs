//! Ports of v12 `grammar/patterns.test.ts` (the `patterns` template, here a
//! function and the `patterns!` macro) and Grammar.test.ts's 'patterns
//! template converts kebab-case to camelCase'.

use cpat_core::{
    Decorators, GrammarError, GrammarOptions, Json, NodeKind, Pattern, patterns, patterns_with,
};

#[test]
fn baseline() {
    let built = patterns(
        r#"
        first-name = "John"
        last-name = "Doe"
        space = /\s+/
        full-name = first-name + space + last-name
        "#,
    )
    .unwrap();
    let mut full_name = built["fullName"].clone();
    let result = full_name.exec("John Doe");
    assert_eq!(result.tree().unwrap().root().value(), "John Doe");
}

#[test]
fn simple_markup() {
    let built = patterns(
        r#"
        tag-name = /[a-zA-Z_-]+[a-zA-Z0-9_-]*/
        space = /\s+/
        opening-tag = "<" + tag-name + space? + ">"
        closing-tag = "</" + tag-name + space? + ">"
        child = space? + element + space?
        children = (child)*
        element = opening-tag + children + closing-tag
        body = space? + element + space?
        "#,
    )
    .unwrap();
    let mut body = built["body"].clone();
    let text = "
        <div>
            <div></div>
            <div></div>
        </div>
        ";
    let result = body.exec_recording(text);
    let tree = result.tree().expect("the markup matches");
    // v12 removes every node named like `space` and reads the rest; the tree
    // here is read-only, so read the leaves that aren't spaces instead.
    let without_spaces: String = tree
        .root()
        .find_all(|node| node.is_leaf() && !node.name().contains("space"))
        .iter()
        .map(|node| node.value())
        .collect();
    assert_eq!(without_spaces, "<div><div></div><div></div></div>");
}

#[test]
fn expression_pattern() {
    let built = patterns(
        r#"
        integer = /\d+/
        operator = "+" | "-" | "*" | "/"
        unary-operator = "+" | "-"
        postfix-operator = "++" | "--"
        binary-expr = expr + operator + expr
        unary-expr = unary-operator + expr
        postfix-expr = expr + postfix-operator
        expr = postfix-expr | unary-expr | binary-expr | integer
        "#,
    )
    .unwrap();
    let mut expr = built["expr"].clone();
    let result = expr.exec("-10++");
    let root = result.tree().unwrap().root();
    assert_eq!(root.value(), "-10++");
    assert_eq!(root.name(), "unary-expr");

    let operator = root.children().next().unwrap();
    assert_eq!(
        (operator.kind(), operator.name(), operator.value()),
        (NodeKind::Literal, "-", "-")
    );
    let operand = root.children().nth(1).unwrap();
    assert_eq!(
        (operand.kind(), operand.name(), operand.value()),
        (NodeKind::Expression, "postfix-expr", "10++")
    );
    let integer = operand.children().next().unwrap();
    assert_eq!(
        (integer.kind(), integer.name(), integer.value()),
        (NodeKind::Regex, "integer", "10")
    );
    let increment = operand.children().nth(1).unwrap();
    assert_eq!(
        (increment.kind(), increment.name(), increment.value()),
        (NodeKind::Literal, "++", "++")
    );
}

#[test]
fn patterns_template_converts_kebab_case_to_camel_case() {
    let built = patterns(r#"my-long-name = "hello""#).unwrap();
    let mut keys: Vec<&str> = built.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(keys, ["myLongName"]);
    let mut pattern = built["myLongName"].clone();
    // The key is camelCase; the pattern keeps its rule's name.
    assert_eq!(pattern.name(), "my-long-name");
    assert_eq!(
        pattern.exec("hello").tree().unwrap().root().value(),
        "hello"
    );
}

#[test]
fn names_that_meet_in_camel_case_keep_the_later_rule() {
    let built = patterns("a-b = \"first\"\naB = \"second\"").unwrap();
    assert_eq!(built.len(), 1);
    assert!(built["aB"].clone().test("second"));
}

#[test]
fn a_grammar_that_does_not_build_is_an_error() {
    assert_eq!(
        patterns("x = missing").unwrap_err(),
        GrammarError::UnknownPattern {
            name: "missing".into(),
            used_in: "x".into(),
        }
    );
}

#[test]
fn the_macro_takes_text_as_it_is() {
    // A grammar's own braces need no escaping when there is nothing to
    // format.
    let built = cpat_core::patterns!(r#"block-of-x = ["{"] "x" ["}"]"#).unwrap();
    assert!(built["blockOfX"].clone().test("{x}"));
}

#[test]
fn the_macro_formats_its_text() {
    let word = "John";
    let built = cpat_core::patterns!(
        "first-name = \"{}\"\nfull-name = first-name + \" {}\"",
        word,
        "Doe"
    )
    .unwrap();
    assert!(built["fullName"].clone().test("John Doe"));
}

#[test]
fn patterns_with_builds_with_its_options() {
    // v12 `createPatternsTemplate(options)`: one function, used again.
    let decorators =
        Decorators::new().with("upper", |pattern: Pattern, _: Option<&Json>| Ok(pattern));
    let options = GrammarOptions {
        decorators,
        params: vec![cpat_core::Literal::new("given", "G").unwrap().into()],
        ..GrammarOptions::default()
    };
    let build = patterns_with(options);
    let first = build("@upper\nx-y = given + \"!\"").unwrap();
    assert!(first["xY"].clone().test("G!"));
    let second = build("@upper\nz = given").unwrap();
    assert!(second["z"].clone().test("G"));
    // Without them, the same text names an unknown decorator.
    assert!(patterns("@upper\nz = given").is_err());
}
