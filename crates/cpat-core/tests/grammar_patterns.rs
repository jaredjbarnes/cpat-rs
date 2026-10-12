//! Ports of v12 `grammar/patterns/grammar.test.ts`: the hand-written cpat
//! grammar's trees. Each test asserts the node values v12's test asserts
//! (`find` is children first, as v12's `Node.find`). Positions are byte
//! offsets; every text here is ASCII.

use cpat_core::{Tree, cpat_grammar};

/// The cpat grammar's tree for `text`; panics when it doesn't match.
fn parse(text: &str) -> Tree<'_> {
    cpat_grammar().exec(text).tree.expect("the text is cpat")
}

/// The value of the first node called `name`, children first.
fn value_of(tree: &Tree<'_>, name: &str) -> Option<String> {
    tree.root()
        .find(|node| node.name() == name)
        .map(|node| node.value().to_string())
}

fn values_of(tree: &Tree<'_>, name: &str) -> Vec<String> {
    tree.root()
        .find_all(|node| node.name() == name)
        .iter()
        .map(|node| node.value().to_string())
        .collect()
}

fn assert_values(tree: &Tree<'_>, expected: &[(&str, &str)]) {
    for (name, value) in expected {
        assert_eq!(
            value_of(tree, name).as_deref(),
            Some(*value),
            "node `{name}`"
        );
    }
}

#[test]
fn syntax_version() {
    let tree = parse("syntax 1.0");
    assert_values(&tree, &[("syntax", "syntax"), ("syntaxVersion", "1.0")]);
}

#[test]
fn import() {
    let tree = parse(r#"import {pattern} from "resource""#);
    assert_values(
        &tree,
        &[
            ("importStatement", r#"import {pattern} from "resource""#),
            ("import", "import"),
            ("patternName", "pattern"),
            ("from", "from"),
            ("resource", r#""resource""#),
        ],
    );
}

#[test]
fn import_with_params() {
    let tree = parse(r#"import {pattern} from "resource" with params {param = "value"}"#);
    assert_values(
        &tree,
        &[
            ("import", "import"),
            ("patternName", "pattern"),
            ("from", "from"),
            ("resource", r#""resource""#),
            ("withParamsExpr", r#"with params {param = "value"}"#),
            ("withParamStatements", r#"param = "value""#),
        ],
    );
    // v12's last assertion looks for a node *named* `param = "value"` and
    // passes on `null` (`toBeDefined`); no node has that name.
    assert_eq!(value_of(&tree, r#"param = "value""#), None);
    // The parameter is a rule of its own inside the block.
    assert_eq!(
        values_of(&tree, "patternAssignment"),
        [r#"param = "value""#]
    );
}

#[test]
fn use_params() {
    let tree = parse("use params { param }");
    assert_values(
        &tree,
        &[
            ("useParamsStatement", "use params { param }"),
            ("useParams", "use params"),
            ("useParamPatterns", "param"),
        ],
    );
}

#[test]
fn literal() {
    let tree = parse(r#"literal = "Hello, world!""#);
    assert_values(
        &tree,
        &[
            ("patternAssignment", r#"literal = "Hello, world!""#),
            ("patternName", "literal"),
            ("literal", r#""Hello, world!""#),
        ],
    );
}

#[test]
fn regex() {
    let tree = parse("regex = /Hello, world!/");
    assert_values(
        &tree,
        &[
            ("patternAssignment", "regex = /Hello, world!/"),
            ("patternName", "regex"),
            ("regex", "/Hello, world!/"),
        ],
    );
}

#[test]
fn regex_stops_at_first_unescaped_slash() {
    let tree = parse(r#"expr = /regex/ + ("Text" | /regex/)"#);
    assert_eq!(values_of(&tree, "regex"), ["/regex/", "/regex/"]);
}

#[test]
fn regex_allows_escaped_slashes() {
    let tree = parse(r"pattern = /a\/b/");
    assert_eq!(value_of(&tree, "regex").as_deref(), Some(r"/a\/b/"));
}

#[test]
fn comment() {
    let tree = parse("# Comment");
    assert_eq!(value_of(&tree, "comment").as_deref(), Some("# Comment"));
}

#[test]
fn sequence() {
    let tree = parse(r#"sequence = "Hello" + "world""#);
    assert_values(
        &tree,
        &[
            ("patternAssignment", r#"sequence = "Hello" + "world""#),
            ("patternName", "sequence"),
            ("sequenceExpr", r#""Hello" + "world""#),
        ],
    );
}

#[test]
fn options() {
    let tree = parse(r#"options = "Hello" | "world""#);
    assert_values(
        &tree,
        &[
            ("patternAssignment", r#"options = "Hello" | "world""#),
            ("patternName", "options"),
            ("optionsExpr", r#""Hello" | "world""#),
        ],
    );
}

#[test]
fn removed_greedy_options_are_still_recognized_so_grammar_can_reject_them() {
    let tree = parse(r#"greedyOptions = "Hello" <|> "world""#);
    assert_eq!(
        value_of(&tree, "removedGreedyOptionsExpr").as_deref(),
        Some(r#""Hello" <|> "world""#)
    );
}

/// `repeat = <expression>`: the assignment, its name and the repeat.
fn assert_repeat(expression: &str) {
    let text = format!("repeat = {expression}");
    let tree = parse(&text);
    assert_values(
        &tree,
        &[
            ("patternAssignment", &text),
            ("patternName", "repeat"),
            ("repeatExpr", expression),
        ],
    );
}

#[test]
fn repeat() {
    assert_repeat("(Hello)+");
}

#[test]
fn repeat_zero_or_more() {
    assert_repeat("(Hello)*");
}

#[test]
fn repeat_with_bounds() {
    assert_repeat("(Hello){2, 4}");
}

#[test]
fn repeat_with_upper_limit() {
    assert_repeat("(Hello){, 4}");
}

#[test]
fn repeat_with_lower_limit() {
    assert_repeat("(Hello){1,}");
}

#[test]
fn repeat_with_delimiter() {
    assert_repeat(r#"(Hello, "world")+"#);
}

#[test]
fn repeat_with_trim_delimiter() {
    assert_repeat(r#"(Hello, "world" trim)+"#);
}

#[test]
fn take_until() {
    let tree = parse(r#"script-text = ?->| "</script""#);
    assert_values(
        &tree,
        &[
            ("takeUntilExpr", r#"?->| "</script""#),
            ("anyChar", "?"),
            ("upTo", "->"),
            ("wall", "|"),
        ],
    );
}

#[test]
fn import_alias() {
    let tree = parse(r#"import {value as alias} from "resource""#);
    assert_values(
        &tree,
        &[
            ("importAlias", "value as alias"),
            ("patternName", "value"),
            ("as", "as"),
            ("importNameAlias", "alias"),
        ],
    );
}

#[test]
fn decorator_without_parens() {
    let tree = parse("@record\nspaces = /\\s+/");
    assert_values(
        &tree,
        &[
            ("nameDecorationStatement", "@record"),
            ("decorationName", "record"),
        ],
    );
}

#[test]
fn decorator_with_empty_args() {
    let tree = parse("@tokens()\nspaces = /\\s+/");
    assert_values(
        &tree,
        &[
            ("methodDecorationStatement", "@tokens()"),
            ("decorationName", "tokens"),
        ],
    );
}

#[test]
fn decorator_with_args() {
    let tree = parse("@tokens([\" \"])\nspaces = /\\s+/");
    assert_values(
        &tree,
        &[
            ("methodDecorationStatement", "@tokens([\" \"])"),
            ("decorationName", "tokens"),
            ("jsonArray", "[\" \"]"),
        ],
    );
}
