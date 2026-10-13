//! Ports of v12 `grammar/Grammar.test.ts`'s decorator tests (DESIGN §9).
//! Messages are v12's, checked against v12 on the same grammars. v12's
//! decorators change the pattern in place; here a decorator returns the
//! pattern to keep, so it may also replace it.

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::*;
use cpat_core::{
    Cursor, Custom, CustomParse, Decorators, Grammar, GrammarError, GrammarOptions, Json, Meta,
    NodeId, NodeKind, Pattern, Patterns, patterns, patterns_with,
};

fn build(text: &str) -> Patterns {
    Grammar::parse_string(text, GrammarOptions::default()).expect("the grammar builds")
}

fn message(text: &str) -> String {
    Grammar::parse_string(text, GrammarOptions::default())
        .expect_err("the grammar is refused")
        .to_string()
}

fn decorated(decorators: Decorators) -> GrammarOptions {
    GrammarOptions {
        decorators,
        ..GrammarOptions::default()
    }
}

type Log = Rc<RefCell<Vec<String>>>;

/// Decorators called `names` that note `name:pattern:argument` and keep the
/// pattern as it is. An absent argument is noted as `undefined`, as v12's
/// JS would print it.
fn recording(names: &[&str]) -> (Decorators, Log) {
    let log = Log::default();
    let decorators = names.iter().fold(Decorators::new(), |decorators, &name| {
        let log = Rc::clone(&log);
        let label = name.to_string();
        decorators.with(name, move |pattern, argument| {
            let argument = argument.map_or("undefined".to_string(), Json::to_string);
            log.borrow_mut()
                .push(format!("{label}:{}:{argument}", pattern.name()));
            Ok(pattern)
        })
    });
    (decorators, log)
}

fn logged(log: &Log) -> Vec<String> {
    log.borrow().clone()
}

// ─── @tokens ────────────────────────────────────────────────────────────

#[test]
fn decorators() {
    let built = patterns(
        r#"
            @tokens([" "])
            spaces = /\s+/
        "#,
    )
    .unwrap();
    assert_eq!(built["spaces"].tokens(), [" "]);
}

#[test]
fn decorators_on_take_until() {
    // The decorator finds its target by capability, so any pattern that
    // accepts tokens can use it, not just a regex.
    let built = patterns(
        r#"
            @tokens(["</script"])
            script-text = ?->| "</script"
        "#,
    )
    .unwrap();
    assert_eq!(built["scriptText"].tokens(), ["</script"]);
}

#[test]
fn tokens_on_a_pattern_that_cant_take_tokens_is_a_build_error() {
    // A literal derives its own token; a sequence offers its children's.
    assert_eq!(
        message(
            r#"
            @tokens(["ignored"])
            name = "John"
        "#
        ),
        "[Invalid Grammar] `@tokens` can't apply to `name` (a literal) — only patterns with \
         setTokens accept tokens"
    );
    assert_eq!(
        message(
            r#"
            @tokens(["x"])
            pair = "a" + "b"
        "#
        ),
        "[Invalid Grammar] `@tokens` can't apply to `pair` (a sequence) — only patterns with \
         setTokens accept tokens"
    );
    // A choice is v12's `expression`; an imported rule arrives as a context.
    assert_eq!(
        message("@tokens([\"x\"])\npair = \"a\" | \"b\""),
        "[Invalid Grammar] `@tokens` can't apply to `pair` (a expression) — only patterns with \
         setTokens accept tokens"
    );
}

#[test]
fn tokens_with_no_argument_is_a_build_error() {
    for text in ["@tokens()\nspaces = /\\s+/", "@tokens\nspaces = /\\s+/"] {
        assert_eq!(
            message(text),
            "[Invalid Grammar] `@tokens` on `spaces` expects an array of strings"
        );
    }
}

#[test]
fn tokens_with_a_non_array_argument_is_a_build_error() {
    assert_eq!(
        message("@tokens(\"Bad\")\nspaces = /\\s+/"),
        "[Invalid Grammar] `@tokens` on `spaces` expects an array of strings"
    );
}

#[test]
fn tokens_with_a_non_string_token_is_a_build_error() {
    let error = Grammar::parse_string(
        "@tokens([\" \", 2])\nspaces = /\\s+/",
        GrammarOptions::default(),
    )
    .unwrap_err();
    assert_eq!(
        error,
        GrammarError::Decorator {
            message: "`@tokens` on `spaces` expects an array of strings".into()
        }
    );
}

#[test]
fn an_unknown_decorator_is_a_build_error() {
    assert_eq!(
        message("@bad-decorator()\nspaces = /\\s+/"),
        "[Invalid Grammar] unknown decorator `@bad-decorator` on `spaces`; known decorators: \
         @tokens."
    );
    assert_eq!(
        message("@tokn\nkeyword = /[a-z]+/"),
        "[Invalid Grammar] unknown decorator `@tokn` on `keyword`; known decorators: @tokens."
    );
}

#[test]
fn an_unknown_decorator_lists_the_known_ones_in_registration_order() {
    // v12 merges `{ ...decorators, ...defaultDecorators }`: the caller's
    // first, and a `tokens` of the caller's keeps its place but not its
    // function.
    let (decorators, _) = recording(&["first", "tokens", "last"]);
    let error =
        Grammar::parse_string("@bad\nspaces = /\\s+/", decorated(decorators.clone())).unwrap_err();
    assert_eq!(
        error.to_string(),
        "[Invalid Grammar] unknown decorator `@bad` on `spaces`; known decorators: @first, \
         @tokens, @last."
    );
    let built =
        Grammar::parse_string("@tokens([\"t\"])\nspaces = /\\s+/", decorated(decorators)).unwrap();
    assert_eq!(built["spaces"].tokens(), ["t"]);
}

#[test]
fn decorators_on_multiple_patterns() {
    let built = patterns(
        r#"
            @tokens([" "])
            spaces = /\s+/

            @tokens(["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"])
            digits = /\d+/
        "#,
    )
    .unwrap();
    assert_eq!(built["spaces"].tokens(), [" "]);
    assert_eq!(
        built["digits"].tokens(),
        ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]
    );
}

#[test]
fn decorators_on_multiple_patterns_and_comments() {
    let built = patterns(
        r#"
            #Comment
            @tokens([" "])
            spaces = /\s+/

            #Comment
            @tokens(["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"])
            #Comment
            digits = /\d+/
        "#,
    )
    .unwrap();
    assert_eq!(built["spaces"].tokens(), [" "]);
    assert_eq!(
        built["digits"].tokens(),
        ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"]
    );
}

// ─── Your own decorators ────────────────────────────────────────────────

#[test]
fn custom_named_decorator() {
    let (decorators, log) = recording(&["record"]);
    patterns_with(decorated(decorators))(
        r#"
            @record
            spaces = /\s+/

            @record
            digits = /\d+/
        "#,
    )
    .unwrap();
    assert_eq!(
        logged(&log),
        ["record:spaces:undefined", "record:digits:undefined"]
    );
}

#[test]
fn decorator_with_empty_object_literal() {
    let (decorators, log) = recording(&["method"]);
    let mut built = patterns_with(decorated(decorators))(
        r#"
            @method({})
            spaces = /\s+/
        "#,
    )
    .unwrap();
    assert_eq!(logged(&log), ["method:spaces:{}"]);
    let spaces = built.get_mut("spaces").unwrap();
    assert_eq!(spaces.name(), "spaces");
    let result = spaces.exec("   ");
    assert_eq!(result.tree().unwrap().root().value(), "   ");
}

#[test]
fn decorator_with_object_literal() {
    let (decorators, log) = recording(&["method"]);
    let mut built = patterns_with(decorated(decorators))(
        r#"
            @method({"prop": 2})
            spaces = /\s+/
        "#,
    )
    .unwrap();
    assert_eq!(logged(&log), [r#"method:spaces:{"prop":2}"#]);
    let spaces = built.get_mut("spaces").unwrap();
    assert_eq!(spaces.name(), "spaces");
    let result = spaces.exec("  ");
    assert_eq!(result.tree().unwrap().root().value(), "  ");
}

#[test]
fn multiple_decorators_applied_to_same_pattern() {
    // v12 walks back from the rule, so the nearest decorator runs first.
    let (decorators, log) = recording(&["first", "second"]);
    patterns_with(decorated(decorators))(
        r#"
            @first
            @second
            name = /\w+/
        "#,
    )
    .unwrap();
    assert_eq!(
        logged(&log),
        ["second:name:undefined", "first:name:undefined"]
    );
}

fn received(text: &str) -> Vec<String> {
    let (decorators, log) = recording(&["config"]);
    patterns_with(decorated(decorators))(text).unwrap();
    logged(&log)
}

#[test]
fn decorator_receives_number_argument() {
    assert_eq!(received("@config(42)\nname = /\\w+/"), ["config:name:42"]);
    assert_eq!(
        received("@config(-1.5)\nname = /\\w+/"),
        ["config:name:-1.5"]
    );
}

#[test]
fn decorator_receives_nested_array_argument() {
    assert_eq!(
        received("@config([1, 2, [3]])\nname = /\\w+/"),
        ["config:name:[1,2,[3]]"]
    );
}

#[test]
fn decorator_receives_null_argument() {
    assert_eq!(
        received("@config(null)\nname = /\\w+/"),
        ["config:name:null"]
    );
}

#[test]
fn decorator_receives_string_argument() {
    assert_eq!(
        received("@config(\"hello\")\nname = /\\w+/"),
        [r#"config:name:"hello""#]
    );
}

#[test]
fn a_boolean_argument_arrives_as_none_as_in_v12() {
    // The cpat grammar's `jsonBoolean` is a choice of two literals, whose
    // node is named after the literal (`true`), so v12 finds no JSON value
    // and passes `undefined`. Kept: grammars written for v12 rely on v12's
    // arguments.
    assert_eq!(
        received("@config(true)\nname = /\\w+/"),
        ["config:name:undefined"]
    );
}

#[test]
fn a_decorator_reaches_only_the_rule_right_below_it() {
    // Comments and blank lines are passed over; any other statement ends
    // the walk, and an export is never decorated.
    assert_eq!(
        received("@config\n\nx = \"a\"\n@config\ny = \"b\"\nz = \"c\""),
        ["config:x:undefined", "config:y:undefined"]
    );
    assert_eq!(
        received("@config\nx = \"a\"\n@config\nx"),
        ["config:x:undefined"]
    );
}

#[test]
fn deviation_an_argument_that_is_cpat_but_not_json_is_a_build_error() {
    // A cpat string may hold escapes JSON doesn't; v12's `JSON.parse` throws
    // V8's own message ("Bad escaped character in JSON at position 3 …").
    let error = Grammar::parse_string(
        "@tokens([\"\\x41\"])\nspaces = /\\s+/",
        GrammarOptions::default(),
    )
    .unwrap_err();
    let GrammarError::DecoratorArgument { decorator, message } = &error else {
        panic!("expected a decorator argument error, got {error:?}");
    };
    assert_eq!(decorator, "tokens");
    assert!(
        error
            .to_string()
            .starts_with("[Invalid Grammar] the argument of `@tokens` isn't JSON: "),
        "{error}"
    );
    assert!(message.contains("escape"), "{message}");
}

// ─── Aliases ────────────────────────────────────────────────────────────

#[test]
fn a_forward_alias_keeps_its_decorators() {
    let mut built = build(
        r#"
            @tokens(["xx"])
            a = b
            b = /x+/
        "#,
    );
    let result = built["a"].exec("xxx");
    assert_eq!(result.tree().unwrap().root().value(), "xxx");
    assert_eq!(built["a"].tokens(), ["xx"]);
    assert!(built["b"].tokens().is_empty());
}

#[test]
fn a_forward_alias_is_decorated_once() {
    let (decorators, log) = recording(&["config"]);
    Grammar::parse_string(
        "@config(1)\na = b\nb = /x+/\n@config(2)\nc = a",
        decorated(decorators),
    )
    .unwrap();
    assert_eq!(logged(&log), ["config:a:1", "config:c:2"]);
}

#[test]
fn a_backward_alias_is_decorated_as_it_is_built() {
    let built = build("b = /x+/\n@tokens([\"xx\"])\na = b");
    assert_eq!(built["a"].tokens(), ["xx"]);
    assert!(built["b"].tokens().is_empty());
}

#[test]
fn a_same_named_param_passed_through_is_decorated() {
    // `s = s` names the param (a literal here), so it is decorated at once
    // rather than waiting to be resolved as an alias.
    let error = Grammar::parse_string(
        "use params { s }\n@tokens([\"q\"])\ns = s",
        GrammarOptions {
            params: vec![lit("s", "s")],
            ..GrammarOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "[Invalid Grammar] `@tokens` can't apply to `s` (a literal) — only patterns with \
         setTokens accept tokens"
    );
}

// ─── Replacing a pattern ────────────────────────────────────────────────

/// Takes one ASCII digit and nothing else.
#[derive(Clone)]
struct Digit;

impl CustomParse for Digit {
    fn parse(&mut self, meta: Meta, cursor: &mut Cursor<'_>) -> Option<NodeId> {
        let start = cursor.pos();
        if !cursor.rest().starts_with(|c: char| c.is_ascii_digit()) {
            cursor.record_error(start, start, meta);
            return None;
        }
        cursor.set_pos(start + 1);
        Some(cursor.leaf(NodeKind::Custom, meta.name, start, start + 1))
    }

    fn clone_box(&self) -> Box<dyn CustomParse> {
        Box::new(self.clone())
    }
}

#[test]
fn a_decorator_can_replace_a_pattern_with_a_custom_one() {
    let decorators = Decorators::new().with("digit", |pattern: Pattern, _: Option<&Json>| {
        Ok(Custom::new(pattern.name(), Box::new(Digit)).into())
    });
    let mut built = Grammar::parse_string(
        r#"
            @digit
            d = "placeholder"
            pair = d + "-" + d
        "#,
        decorated(decorators),
    )
    .unwrap();

    // `pair` was built after `d` was replaced, and finds it by name.
    let result = built["pair"].exec("4-2");
    let root = result.tree().expect("the custom pattern matches").root();
    let parts: Vec<(NodeKind, &str, &str)> = root
        .children()
        .map(|node| (node.kind(), node.name(), node.value()))
        .collect();
    assert_eq!(
        parts,
        [
            (NodeKind::Custom, "d", "4"),
            (NodeKind::Literal, "-", "-"),
            (NodeKind::Custom, "d", "2"),
        ]
    );
    assert!(built["pair"].exec("placeholder-4").tree().is_none());
    assert!(built["d"].test("7"));
}

#[test]
fn a_decorator_can_replace_a_pattern_a_rule_refers_to_before_it_is_defined() {
    let decorators = Decorators::new().with("digit", |pattern: Pattern, _: Option<&Json>| {
        Ok(Custom::new(pattern.name(), Box::new(Digit)).into())
    });
    let mut built = Grammar::parse_string(
        "pair = d + \"-\" + d\n@digit\nd = \"placeholder\"",
        decorated(decorators),
    )
    .unwrap();
    assert!(built["pair"].test("4-2"));
}

#[test]
fn a_decorators_error_is_a_build_error() {
    let decorators = Decorators::new().with("never", |pattern: Pattern, _: Option<&Json>| {
        Err(format!("`{}` can't be decorated", pattern.name()))
    });
    let error = Grammar::parse_string("@never\nx = \"a\"", decorated(decorators)).unwrap_err();
    assert_eq!(
        error.to_string(),
        "[Invalid Grammar] `x` can't be decorated"
    );
}

#[test]
fn a_decorator_that_renames_its_pattern_is_a_build_error() {
    let decorators = Decorators::new().with("rename", |pattern: Pattern, _: Option<&Json>| {
        Ok(pattern.clone_named("other"))
    });
    let error = Grammar::parse_string("@rename\nx = \"a\"", decorated(decorators)).unwrap_err();
    assert_eq!(
        error,
        GrammarError::DecoratorRenamed {
            decorator: "rename".into(),
            from: "x".into(),
            to: "other".into(),
        }
    );
    assert_eq!(
        error.to_string(),
        "[Invalid Grammar] `@rename` renamed `x` to `other`; a decorator must keep the \
         pattern's name, since other rules find it by that name."
    );
}

#[test]
fn an_imported_grammar_uses_the_importers_decorators() {
    let (decorators, log) = recording(&["config"]);
    let options = GrammarOptions {
        decorators,
        ..GrammarOptions::default()
    }
    .with_resolver(cpat_core::InMemoryImports::new().with("lib", "@config(1)\na = \"a\""));
    Grammar::parse_string("import { a } from \"lib\"\n@config(2)\nx = a", options).unwrap();
    assert_eq!(logged(&log), ["config:a:1", "config:x:2"]);
}
