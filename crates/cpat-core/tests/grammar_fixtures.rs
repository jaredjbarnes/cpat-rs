//! Real grammars through the builder.
//!
//! `fixtures/harness/` holds the 11 `.cpat` grammars of the agent-harness
//! package (shell, glob, calc, find, jq, saya, sed, date expressions, and
//! the BRE/ERE/PCRE regex dialects), copied as they were at agent-harness
//! commit bef209c. Each must build, and its entry rule must parse inputs
//! like the package's tests use to the tree v12 makes:
//! `fixtures/harness_smoke.tsv` was written by running v12 on them.
//!
//! The self-host test: `fixtures/cpat.cpat` is cpat's own grammar written
//! in cpat. Built by the builder, it must parse every grammar here to the
//! same tree as the hand-written grammar (`cpat_grammar`).

use std::path::PathBuf;

use cpat_core::{Grammar, GrammarOptions, Node, Pattern, Patterns, cpat_grammar};

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn read(path: PathBuf) -> String {
    std::fs::read_to_string(&path).unwrap_or_else(|error| panic!("{path:?}: {error}"))
}

/// Every harness grammar, as `(file name, source)`, in name order.
fn harness_grammars() -> Vec<(String, String)> {
    let mut grammars: Vec<(String, String)> = std::fs::read_dir(fixtures().join("harness"))
        .expect("the harness fixtures exist")
        .map(|entry| {
            let path = entry.expect("a readable entry").path();
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, read(path))
        })
        .collect();
    grammars.sort();
    grammars
}

fn build(source: &str) -> Patterns {
    Grammar::parse_string(source, GrammarOptions::default())
        .unwrap_or_else(|error| panic!("the grammar builds: {error}"))
}

/// `name@start..end[children]`, walked without recursion (an operator
/// chain makes a tree as deep as it is long). `rename` maps node names.
fn outline(root: Node<'_, '_>, rename: &dyn Fn(&str) -> String) -> String {
    enum Step<'a, 't> {
        Open(Node<'a, 't>),
        Text(&'static str),
    }
    let mut out = String::new();
    let mut pending = vec![Step::Open(root)];
    while let Some(step) = pending.pop() {
        match step {
            Step::Text(text) => out.push_str(text),
            Step::Open(node) => {
                out.push_str(&format!(
                    "{}@{}..{}",
                    rename(node.name()),
                    node.start(),
                    node.end()
                ));
                if node.children().len() > 0 {
                    out.push('[');
                    pending.push(Step::Text("]"));
                    let children: Vec<Node<'_, '_>> = node.children().collect();
                    for (i, child) in children.into_iter().enumerate().rev() {
                        pending.push(Step::Open(child));
                        if i > 0 {
                            pending.push(Step::Text(" "));
                        }
                    }
                }
            }
        }
    }
    out
}

fn same_name(name: &str) -> String {
    name.to_string()
}

// ─── The harness grammars ───────────────────────────────────────────────

#[test]
fn every_harness_grammar_builds() {
    let grammars = harness_grammars();
    assert_eq!(grammars.len(), 11);
    // Rule counts as v12 builds them.
    let counts: Vec<(String, usize)> = grammars
        .iter()
        .map(|(name, source)| (name.clone(), build(source).len()))
        .collect();
    let expected = [
        ("bre.cpat", 35),
        ("calc.cpat", 78),
        ("date_expr.cpat", 51),
        ("ere.cpat", 35),
        ("find.cpat", 60),
        ("glob.cpat", 21),
        ("jq.cpat", 200),
        ("pcre.cpat", 48),
        ("saya.cpat", 197),
        ("sed.cpat", 43),
        ("shell.cpat", 345),
    ];
    let expected: Vec<(String, usize)> = expected
        .iter()
        .map(|(name, count)| (name.to_string(), *count))
        .collect();
    assert_eq!(counts, expected);
}

/// A JSON string literal's text (the smoke fixture quotes its inputs).
fn json_string(quoted: &str) -> String {
    let body = &quoted[1..quoted.len() - 1];
    let mut text = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            text.push(c);
            continue;
        }
        match chars.next().expect("an escape has a letter") {
            'n' => text.push('\n'),
            'r' => text.push('\r'),
            't' => text.push('\t'),
            'u' => {
                let hex: String = chars.by_ref().take(4).collect();
                let unit = u32::from_str_radix(&hex, 16).expect("four hex digits");
                text.push(char::from_u32(unit).expect("not a surrogate"));
            }
            other => text.push(other),
        }
    }
    text
}

#[test]
fn every_harness_entry_rule_parses_its_smoke_inputs_as_v12_does() {
    let smoke = read(fixtures().join("harness_smoke.tsv"));
    let mut built: Vec<(String, Patterns)> = Vec::new();
    let mut checked = 0;

    for line in smoke.lines().filter(|line| !line.starts_with('#')) {
        let fields: Vec<&str> = line.split('\t').collect();
        let [fixture, rule, input, expected] = fields[..] else {
            panic!("a smoke line has four fields: {line}");
        };
        let input = json_string(input);

        if built.last().is_none_or(|(name, _)| name != fixture) {
            let source = read(fixtures().join("harness").join(fixture));
            built.push((fixture.to_string(), build(&source)));
        }
        let patterns = &mut built.last_mut().unwrap().1;

        let result = patterns[rule].exec(&input);
        let tree = result
            .tree()
            .unwrap_or_else(|| panic!("{fixture} `{rule}` parses {input:?}"));
        assert_eq!(
            outline(tree.root(), &same_name),
            expected,
            "{fixture} `{rule}` on {input:?}"
        );
        checked += 1;
    }

    // Every grammar, with its entry rule.
    let entries: Vec<&str> = built.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(entries.len(), 11);
    assert_eq!(checked, 44);
}

// ─── Self-host ──────────────────────────────────────────────────────────

/// The comma is the one node a grammar file can't name as v12's grammar
/// does (`,`, a regex; a rule name must be a word): `fixtures/cpat.cpat`
/// calls it `comma`.
fn as_hand_written(name: &str) -> String {
    match name {
        "comma" => ",".to_string(),
        other => other.to_string(),
    }
}

/// The corpus: cpat's own grammar file and every harness grammar, plus
/// small grammars that use the rest of the syntax.
fn corpus() -> Vec<(String, String)> {
    let mut corpus = vec![("cpat.cpat".to_string(), read(fixtures().join("cpat.cpat")))];
    corpus.extend(harness_grammars());
    let snippets = [
        r#"import {pattern, other as alias} from "resource" with params {param = "value"}"#,
        "use params { a, b = c }",
        "@tokens([\" \", 1.5, true, null, {\"a\": [false]}])\n@record\nspaces = /\\s+/",
        r#"x = !"a" + "b"? + c right | (d, "," trim){2, 3} | ?->| "e" | ["{"] ... ["}"]"#,
        r#"x = ["("] "yes" | "no" [")"] + ("a"){,4} + ("b"){1,}"#,
        r#"x = "a" ~/ "b" | "a" <|> "c""#,
        "syntax 1.0\n\n# comment\nx = y\nx\n",
    ];
    corpus.extend(
        snippets
            .iter()
            .enumerate()
            .map(|(i, text)| (format!("snippet {i}"), text.to_string())),
    );
    corpus
}

/// The cpat grammar as the builder makes it from `fixtures/cpat.cpat`.
fn self_hosted() -> Pattern {
    build(&read(fixtures().join("cpat.cpat")))
        .remove("cpat")
        .expect("cpat.cpat defines `cpat`")
}

#[test]
fn the_built_cpat_grammar_parses_the_corpus_as_the_hand_written_one() {
    let mut hand_written = cpat_grammar();
    let mut built = self_hosted();

    for (name, text) in corpus() {
        let expected = hand_written.exec(&text);
        let actual = built.exec(&text);
        let expected = expected
            .tree()
            .unwrap_or_else(|| panic!("the hand-written grammar parses {name}"));
        let actual = actual
            .tree()
            .unwrap_or_else(|| panic!("the built grammar parses {name}"));
        assert_eq!(
            outline(actual.root(), &as_hand_written),
            outline(expected.root(), &same_name),
            "{name}"
        );
    }
}

#[test]
fn the_built_cpat_grammar_refuses_what_the_hand_written_one_refuses() {
    let mut hand_written = cpat_grammar();
    let mut built = self_hosted();

    for text in [
        "",
        "//",
        "x = ",
        "x = \"a\" +",
        "x = a | | b",
        "x = [\"{\"] [\"}\"]",
    ] {
        let expected = hand_written.exec(text);
        let actual = built.exec(text);
        assert!(expected.tree().is_none(), "{text:?}");
        assert!(actual.tree().is_none(), "{text:?}");
        // Both give up at the same place.
        assert_eq!(
            actual.leaf_match.map(|leaf| leaf.end),
            expected.leaf_match.map(|leaf| leaf.end),
            "{text:?}"
        );
    }
}

#[test]
fn the_built_cpat_grammar_builds_itself() {
    // A grammar built from its own text by the builder it feeds: the
    // patterns it produces from cpat.cpat parse cpat.cpat again.
    let source = read(fixtures().join("cpat.cpat"));
    let mut built = self_hosted();
    let result = built.exec(&source);
    let tree = result.tree().expect("cpat.cpat is cpat");
    assert_eq!(tree.root().value(), source);
    let rules = tree
        .root()
        .find_all(|node| node.name() == "patternAssignment")
        .len();
    assert_eq!(rules, build(&source).len());
}
