//! Ports of v12 `grammar/Grammar.test.ts`'s import and param tests, with an
//! in-memory resolver. v12's async tests (`Grammar.parse`, `Grammar.import`
//! with `resolveImport`) are synchronous here: the core never fetches.
//! Error tests assert v12's message text exactly; each message was checked
//! against v12 on the same grammars.

mod common;

use std::cell::RefCell;
use std::rc::Rc;

use common::*;
use cpat_core::{
    Grammar, GrammarError, GrammarFile, GrammarOptions, InMemoryImports, Pattern, Patterns,
};

fn options(files: &[(&str, &str)]) -> GrammarOptions {
    let imports = files
        .iter()
        .fold(InMemoryImports::new(), |imports, (resource, text)| {
            imports.with(resource, text)
        });
    GrammarOptions::default().with_resolver(imports)
}

fn with_params(files: &[(&str, &str)], params: Vec<Pattern>) -> GrammarOptions {
    GrammarOptions {
        params,
        ..options(files)
    }
}

fn build(text: &str, options: GrammarOptions) -> Patterns {
    Grammar::parse_string(text, options).expect("the grammar builds")
}

fn message(text: &str, options: GrammarOptions) -> String {
    Grammar::parse_string(text, options)
        .expect_err("the grammar is refused")
        .to_string()
}

/// The value `rule` matches when it runs over all of `text`.
fn value(patterns: &mut Patterns, rule: &str, text: &str) -> Option<String> {
    let result = patterns[rule].exec(text);
    result.tree().map(|tree| tree.root().value().to_string())
}

// ─── Imports ────────────────────────────────────────────────────────────

#[test]
fn import() {
    let mut patterns = build(
        r#"
        import { first-name } from "some/path/to/file.cpat"
        last-name = "Doe"
        space = " "
        full-name = first-name + space + last-name
        "#,
        options(&[("some/path/to/file.cpat", r#"first-name = "John""#)]),
    );
    assert_eq!(
        value(&mut patterns, "full-name", "John Doe").as_deref(),
        Some("John Doe")
    );
}

#[test]
fn imports() {
    let mut patterns = build(
        r#"
        import { first-name } from "first-name.cpat"
        import { space } from "space.cpat"
        last-name = "Doe"
        full-name = first-name + space + last-name
        "#,
        options(&[
            ("space.cpat", r#"space = " ""#),
            ("first-name.cpat", r#"first-name = "John""#),
        ]),
    );
    assert_eq!(
        value(&mut patterns, "full-name", "John Doe").as_deref(),
        Some("John Doe")
    );
}

#[test]
fn imports_with_params() {
    let mut patterns = build(
        r#"
        import { first-name } from "first-name.cpat"
        import { space } from "space.cpat" with params {
          custom-space = "  "
        }
        last-name = "Doe"
        full-name = first-name + space + last-name
        "#,
        options(&[
            (
                "space.cpat",
                "
        use params { custom-space }
        space = custom-space
        ",
            ),
            ("first-name.cpat", r#"first-name = "John""#),
        ]),
    );
    assert_eq!(
        value(&mut patterns, "full-name", "John  Doe").as_deref(),
        Some("John  Doe")
    );
    // The param replaced the space: one space no longer does.
    assert_eq!(value(&mut patterns, "full-name", "John Doe"), None);
}

#[test]
fn export_name() {
    let mut patterns = build(
        "
        import { use-this } from \"resource1\"
        import {name} from \"resource2\" with params {
           use-this
        }

        name
        ",
        options(&[
            ("resource1", "\n        use-this = \"Use This\"\n        "),
            (
                "resource2",
                "
        use params {
            use-this
        }
        name = use-this
        ",
            ),
        ]),
    );
    assert_eq!(
        value(&mut patterns, "name", "Use This").as_deref(),
        Some("Use This")
    );
}

#[test]
fn import_alias() {
    let mut patterns = build(
        r#"
        import { value as alias } from "resource1"
        import { export-value } from "resource2" with params {
            param = alias
        }
        name = export-value
        "#,
        options(&[
            ("resource1", "\n        value = \"Value\"\n        "),
            (
                "resource2",
                "
        use params {
            param
        }
        export-value = param
        ",
            ),
        ]),
    );
    assert_eq!(
        value(&mut patterns, "name", "Value").as_deref(),
        Some("Value")
    );
}

#[test]
fn an_imported_alias_is_a_clone_under_the_alias_name() {
    let patterns = build(
        r#"
        import { value as alias } from "resource1"
        name = alias + "!"
        "#,
        options(&[("resource1", r#"value = "Value""#)]),
    );
    let Pattern::Context(name) = &patterns["name"] else {
        panic!("a rule is a context");
    };
    let Pattern::Sequence(sequence) = name.child() else {
        panic!("`name` is a sequence");
    };
    assert_eq!(sequence.children()[0].name(), "alias");
    let mut name = patterns["name"].clone();
    let result = name.exec("Value!");
    let first = result.tree().unwrap().root().children().next().unwrap();
    assert_eq!((first.name(), first.value()), ("alias", "Value"));
}

const DEFAULT_PARAMS: &str = r#"
        use params {
            value = default-value
        }

        default-value = "DefaultValue"
        alias = value
        "#;

#[test]
fn default_params_resolves_to_default_value() {
    let mut patterns = build(DEFAULT_PARAMS, options(&[]));
    assert_eq!(
        value(&mut patterns, "alias", "DefaultValue").as_deref(),
        Some("DefaultValue")
    );
}

#[test]
fn default_params_resolves_to_params_imported() {
    let mut patterns = build(
        DEFAULT_PARAMS,
        with_params(&[], vec![lit("value", "Value")]),
    );
    assert_eq!(
        value(&mut patterns, "alias", "Value").as_deref(),
        Some("Value")
    );
    assert_eq!(value(&mut patterns, "alias", "DefaultValue"), None);
}

#[test]
fn default_params_resolves_to_imported_default_value() {
    let mut patterns = build(
        r#"
        import { my-value as default-value } from "resource1"
        use params {
            value = default-value
        }

        default-value = "DefaultValue"
        alias = value
        "#,
        options(&[(
            "resource1",
            "\n            my-value = \"MyValue\"\n        ",
        )]),
    );
    assert_eq!(
        value(&mut patterns, "alias", "MyValue").as_deref(),
        Some("MyValue")
    );
    assert_eq!(value(&mut patterns, "alias", "DefaultValue"), None);
}

#[test]
fn grammar_import() {
    let root = r#"
        use params {
          custom-space
        }
        import { first-name } from "first-name.cpat"
        import { space } from "space.cpat" with params {
          custom-space = custom-space
        }
        last-name = "Doe"
        full-name = first-name + space + last-name
        "#;
    let space = "
        use params { custom-space }
        space = custom-space
        ";
    let mut patterns = Grammar::import(
        "root.cpat",
        with_params(
            &[
                ("space.cpat", space),
                ("first-name.cpat", r#"first-name = "John""#),
                ("root.cpat", root),
            ],
            vec![lit("custom-space", "  ")],
        ),
    )
    .expect("the grammar builds");
    assert_eq!(
        value(&mut patterns, "full-name", "John  Doe").as_deref(),
        Some("John  Doe")
    );
}

#[test]
fn grammar_import_passes_on_the_resolvers_error() {
    let error = Grammar::import("missing.cpat", options(&[])).unwrap_err();
    assert_eq!(
        error.to_string(),
        "No grammar is held for \"missing.cpat\"; add it with `InMemoryImports::with`."
    );
    assert_eq!(
        Grammar::import("root.cpat", GrammarOptions::default()).unwrap_err(),
        GrammarError::NoImportResolver
    );
}

#[test]
fn import_sync() {
    let mut patterns = build(
        r#"
        import { name } from "first-name.cpat"
        full-name = name
        "#,
        GrammarOptions {
            origin_resource: Some("/root.cpat".into()),
            ..options(&[
                (
                    "first-name.cpat",
                    r#"
        import { last-name } from "last-name.cpat"
        first-name = "John"
        name = first-name + " " +  last-name
        "#,
                ),
                ("last-name.cpat", "\n        last-name = \"Doe\"\n        "),
            ])
        },
    );
    assert_eq!(
        value(&mut patterns, "full-name", "John Doe").as_deref(),
        Some("John Doe")
    );
}

type Seen = Rc<RefCell<Vec<(String, Option<String>)>>>;

/// A resolver that notes what it was asked, and names every grammar it
/// returns in upper case, so a test can tell the asked-for name from the
/// one it answered with.
fn spying(files: &'static [(&'static str, &'static str)]) -> (GrammarOptions, Seen) {
    let seen = Seen::default();
    let noted = Rc::clone(&seen);
    let resolver = move |resource: &str, origin: Option<&str>| {
        noted
            .borrow_mut()
            .push((resource.to_string(), origin.map(String::from)));
        let (_, expression) = files
            .iter()
            .find(|(name, _)| *name == resource)
            .expect("the test holds every grammar");
        Ok(GrammarFile {
            resource: resource.to_uppercase(),
            expression: expression.to_string(),
        })
    };
    (GrammarOptions::default().with_resolver(resolver), seen)
}

const CHAIN: &[(&str, &str)] = &[
    (
        "first-name.cpat",
        "import { last-name } from \"last-name.cpat\"\nfirst-name = \"John\"\nname = first-name + \" \" + last-name",
    ),
    ("last-name.cpat", "last-name = \"Doe\""),
    ("p.cpat", "use params { q }\np = q"),
    (
        "root.cpat",
        "import { name } from \"first-name.cpat\"\nfull-name = name",
    ),
];

fn asked(seen: &Seen) -> Vec<(String, Option<String>)> {
    seen.borrow().clone()
}

fn pair(resource: &str, origin: Option<&str>) -> (String, Option<String>) {
    (resource.to_string(), origin.map(String::from))
}

#[test]
fn each_import_is_resolved_from_the_resource_its_grammar_came_from() {
    // Checked against v12: the origin is the resource the resolver
    // answered with, and the top-level text's is `origin_resource`.
    let (options, seen) = spying(CHAIN);
    let text = "import { name } from \"first-name.cpat\"\nfull-name = name";
    let options = GrammarOptions {
        origin_resource: Some("/root.cpat".into()),
        ..options
    };
    build(text, options);
    assert_eq!(
        asked(&seen),
        [
            pair("first-name.cpat", Some("/root.cpat")),
            pair("last-name.cpat", Some("FIRST-NAME.CPAT")),
        ]
    );

    let (options, seen) = spying(CHAIN);
    build(text, options);
    assert_eq!(asked(&seen)[0], pair("first-name.cpat", None));

    // An empty origin is none, as v12's `originResource || null` makes it.
    let (options, seen) = spying(CHAIN);
    build(
        text,
        GrammarOptions {
            origin_resource: Some(String::new()),
            ..options
        },
    );
    assert_eq!(asked(&seen)[0], pair("first-name.cpat", None));
}

#[test]
fn a_with_params_block_resolves_from_its_importers_origin() {
    let (options, seen) = spying(CHAIN);
    let mut patterns = build(
        "import { last-name } from \"last-name.cpat\"\nimport { p } from \"p.cpat\" with params { q = last-name }\nx = p",
        GrammarOptions {
            origin_resource: Some("here".into()),
            ..options
        },
    );
    assert_eq!(
        asked(&seen),
        [
            pair("last-name.cpat", Some("here")),
            pair("p.cpat", Some("here"))
        ]
    );
    assert_eq!(value(&mut patterns, "x", "Doe").as_deref(), Some("Doe"));
}

#[test]
fn grammar_import_resolves_the_root_with_no_origin() {
    let (options, seen) = spying(CHAIN);
    Grammar::import("root.cpat", options).expect("the grammar builds");
    assert_eq!(
        asked(&seen),
        [
            pair("root.cpat", None),
            pair("first-name.cpat", Some("ROOT.CPAT")),
            pair("last-name.cpat", Some("FIRST-NAME.CPAT")),
        ]
    );
}

#[test]
fn import_multiple_patterns_from_same_resource() {
    let mut patterns = build(
        r#"
            import { first, last } from "names.cpat"
            full = first + " " + last
        "#,
        options(&[("names.cpat", "first = \"John\"\nlast = \"Doe\"")]),
    );
    assert_eq!(
        value(&mut patterns, "full", "John Doe").as_deref(),
        Some("John Doe")
    );
}

#[test]
fn import_multiple_patterns_with_alias_from_same_resource() {
    let mut patterns = build(
        r#"
            import { first, last as surname } from "names.cpat"
            full = first + " " + surname
        "#,
        options(&[("names.cpat", "first = \"John\"\nlast = \"Doe\"")]),
    );
    assert_eq!(
        value(&mut patterns, "full", "John Doe").as_deref(),
        Some("John Doe")
    );
}

#[test]
fn export_pattern_promotes_previously_defined_pattern() {
    let mut patterns = build(
        r#"
            import { value } from "r1"
            value
        "#,
        options(&[("r1", r#"value = "Hello""#)]),
    );
    assert_eq!(patterns.names().collect::<Vec<_>>(), ["value"]);
    assert_eq!(
        value(&mut patterns, "value", "Hello").as_deref(),
        Some("Hello")
    );
}

#[test]
fn an_imports_anonymous_names_do_not_collide_with_the_importers() {
    // v12 'an imported grammar's anonymous names do not collide with the
    // importer's': with a count per grammar, `pair`'s inner choice and `x`'s
    // would share a name, and `x`'s choice would take `pair` for itself.
    let mut patterns = build(
        r#"
            import { pair } from "lib.cpat"
            x = (pair | "z") + ("y" | "w")
        "#,
        options(&[("lib.cpat", r#"pair = ("q" | "r") + "s""#)]),
    );
    assert_eq!(value(&mut patterns, "x", "qsy").as_deref(), Some("qsy"));
    assert_eq!(value(&mut patterns, "x", "zw").as_deref(), Some("zw"));
}

#[test]
fn an_import_continues_the_importers_anonymous_count() {
    let patterns = build(
        r#"
            import { pair } from "lib.cpat"
            x = ("a" | "b") + "c"
        "#,
        options(&[("lib.cpat", r#"pair = ("q" | "r") + "s""#)]),
    );
    let Pattern::Context(x) = &patterns["x"] else {
        panic!("a rule is a context");
    };
    let Pattern::Sequence(sequence) = x.child() else {
        panic!("`x` is a sequence");
    };
    // The import took `anonymous-pattern-0` to `-4` (as v12 numbers them),
    // and `x`'s group `-5`; its choice is next.
    assert_eq!(sequence.children()[0].name(), "anonymous-pattern-6");
}

#[test]
fn an_import_a_rule_redefines_loses_to_the_rule_once_it_is_built() {
    // v12 looks a name up in the rules built so far, then the imports: a
    // rule using `a` before the grammar redefines it gets the import.
    let lib = [("lib", r#"a = "lib""#)];
    let mut later = build(
        "import { a } from \"lib\"\nx = a\na = \"local\"",
        options(&lib),
    );
    assert_eq!(value(&mut later, "x", "lib").as_deref(), Some("lib"));
    assert_eq!(value(&mut later, "x", "local"), None);

    let mut earlier = build(
        "import { a } from \"lib\"\na = \"local\"\nx = a",
        options(&lib),
    );
    assert_eq!(value(&mut earlier, "x", "local").as_deref(), Some("local"));
    assert_eq!(value(&mut earlier, "x", "lib"), None);
}

// ─── Params ─────────────────────────────────────────────────────────────

#[test]
fn default_param_resolves_to_locally_defined_pattern_when_no_param_supplied() {
    let mut patterns = build(
        r#"
            use params { value = default-value }
            default-value = "DefaultValue"
            alias = value
        "#,
        GrammarOptions::default(),
    );
    assert_eq!(
        value(&mut patterns, "alias", "DefaultValue").as_deref(),
        Some("DefaultValue")
    );
}

#[test]
fn default_param_is_overridden_when_param_is_supplied() {
    let mut patterns = build(
        r#"
            use params { value = default-value }
            default-value = "DefaultValue"
            alias = value
        "#,
        with_params(&[], vec![lit("value", "Supplied")]),
    );
    assert_eq!(
        value(&mut patterns, "alias", "Supplied").as_deref(),
        Some("Supplied")
    );
}

#[test]
fn default_param_resolves_to_imported_pattern() {
    let mut patterns = build(
        r#"
            import { my-value as default-value } from "r1"
            use params { value = default-value }
            alias = value
        "#,
        options(&[("r1", r#"my-value = "MyValue""#)]),
    );
    assert_eq!(
        value(&mut patterns, "alias", "MyValue").as_deref(),
        Some("MyValue")
    );
}

#[test]
fn use_params_without_default_declares_param_available_from_options() {
    let mut patterns = build(
        r#"
            use params { value }
            alias = value
        "#,
        with_params(&[], vec![lit("value", "Provided")]),
    );
    assert_eq!(
        value(&mut patterns, "alias", "Provided").as_deref(),
        Some("Provided")
    );
}

#[test]
fn a_use_params_default_that_names_nothing_is_an_error() {
    assert_eq!(
        message(
            r#"
            use params { value = nowhere }
            x = value
        "#,
            GrammarOptions::default()
        ),
        "[Invalid Grammar] unknown pattern `nowhere` used in `use params { value }`; define it \
         in this grammar, import it, or pass it as a param."
    );
}

#[test]
fn a_declared_param_that_is_neither_supplied_nor_defaulted_is_an_error_when_used() {
    let text = r#"
            use params { value }
            x = value + "!"
        "#;
    assert_eq!(
        message(text, GrammarOptions::default()),
        "[Invalid Grammar] unknown pattern `value` used in `x`; define it in this grammar, \
         import it, or pass it as a param."
    );

    let mut patterns = build(text, with_params(&[], vec![lit("value", "v")]));
    assert_eq!(value(&mut patterns, "x", "v!").as_deref(), Some("v!"));
}

#[test]
fn imported_and_param_names_count_as_defined() {
    let mut patterns = build(
        r#"
            import { a } from "lib"
            x = a + b
        "#,
        with_params(&[("lib", r#"a = "A""#)], vec![lit("b", "B")]),
    );
    assert_eq!(value(&mut patterns, "x", "AB").as_deref(), Some("AB"));
}

#[test]
fn a_rule_may_pass_a_same_named_param_through() {
    // `space = space` names the param, not itself: it is no alias cycle.
    let mut patterns = build(
        r#"
            use params { space }
            space = space
        "#,
        with_params(&[], vec![lit("space", " ")]),
    );
    assert_eq!(value(&mut patterns, "space", " ").as_deref(), Some(" "));
}

#[test]
fn a_with_params_block_sees_the_importers_imports_and_params() {
    let lib = ("lib", "use params { p }\nx = p + \"!\"");
    let mut imported = build(
        "import { q } from \"q\"\nimport { x } from \"lib\" with params { p = q }\ny = x",
        options(&[("q", r#"q = "Q""#), lib]),
    );
    assert_eq!(value(&mut imported, "y", "Q!").as_deref(), Some("Q!"));

    let mut given = build(
        "import { x } from \"lib\" with params { p = given }\ny = x",
        with_params(&[lib], vec![lit("given", "G")]),
    );
    assert_eq!(value(&mut given, "y", "G!").as_deref(), Some("G!"));
}

#[test]
fn a_with_params_block_may_name_the_importers_rules() {
    // The importer's rules are built after its imports, so they can't be
    // params yet; v12 counts them as defined, and they resolve when parsed.
    let lib = ("lib", "use params { p }\nx = p + \"!\"");
    let mut patterns = build(
        "import { x } from \"lib\" with params { p = my-rule }\nmy-rule = \"a\"\ny = x",
        options(&[lib]),
    );
    assert_eq!(value(&mut patterns, "y", "a!").as_deref(), Some("a!"));
    assert_eq!(value(&mut patterns, "y", "b!"), None);

    assert_eq!(
        message(
            "import { x } from \"lib\" with params { p = zz }\ny = x",
            options(&[lib])
        ),
        "[Invalid Grammar] unknown pattern `zz` used in `p`; define it in this grammar, import \
         it, or pass it as a param."
    );
}

#[test]
fn a_param_passes_through_to_an_import() {
    let mut patterns = build(
        "use params { sp }\nimport { a } from \"lib\" with params { sp = sp }\nx = a",
        with_params(
            &[("lib", "use params { sp }\na = \"a\" + sp")],
            vec![lit("sp", "_")],
        ),
    );
    assert_eq!(value(&mut patterns, "x", "a_").as_deref(), Some("a_"));
}

// ─── Mistakes ───────────────────────────────────────────────────────────

#[test]
fn looks_through_imported_literals() {
    assert_eq!(
        message(
            r#"
            import { lt } from "ops"
            op = lt | "<="
        "#,
            options(&[("ops", r#"lt = "<""#)])
        ),
        r#"[Invalid Grammar] `"<="` can never match after `"<"` in `op`; put the longer literal first."#
    );
}

#[test]
fn duplicate_import_name_throws() {
    let error = Grammar::parse_string(
        r#"
            import { value } from "r1"
            import { value } from "r2"
        "#,
        options(&[("r1", r#"value = "A""#), ("r2", r#"value = "A""#)]),
    )
    .unwrap_err();
    assert_eq!(
        error,
        GrammarError::ImportFailed {
            resource: "r2".into(),
            details: Box::new(GrammarError::ImportNameTaken {
                name: "value".into()
            }),
        }
    );
    assert_eq!(
        error.to_string(),
        "Failed loading expression from: \"r2\". Error details: \"'value' was already used \
         within another import.\""
    );
}

#[test]
fn duplicate_import_alias_throws() {
    assert_eq!(
        message(
            r#"
            import { a as alias } from "r1"
            import { b as alias } from "r2"
        "#,
            options(&[("r1", r#"a = "A""#), ("r2", r#"b = "B""#)])
        ),
        "Failed loading expression from: \"r2\". Error details: \"'alias' was already used \
         within another import.\""
    );
}

#[test]
fn importing_nonexistent_pattern_throws() {
    assert_eq!(
        message(
            r#"
            import { nonexistent } from "r1"
        "#,
            options(&[("r1", r#"value = "A""#)])
        ),
        "Failed loading expression from: \"r1\". Error details: \"Couldn't find pattern with \
         name: nonexistent, from import: r1.\""
    );
}

#[test]
fn importing_nonexistent_pattern_via_alias_throws() {
    assert_eq!(
        message(
            r#"
            import { nonexistent as alias } from "r1"
        "#,
            options(&[("r1", r#"value = "A""#)])
        ),
        "Failed loading expression from: \"r1\". Error details: \"Couldn't find pattern with \
         name: nonexistent, from import: r1.\""
    );
}

#[test]
fn import_without_resolver_throws() {
    // v12 'import without resolver (sync) throws' and '(async) throws'.
    let text = r#"
            import { value } from "file.cpat"
            name = value
        "#;
    assert_eq!(
        Grammar::parse_string(text, GrammarOptions::default()).unwrap_err(),
        GrammarError::NoImportResolver
    );
    assert_eq!(
        message(text, GrammarOptions::default()),
        "No import resolver supplied."
    );
}

#[test]
fn import_wraps_error_from_bad_imported_grammar() {
    // v12 'sync import wraps error from bad imported grammar' and 'async …'.
    assert_eq!(
        message(
            r#"
            import { value } from "bad.cpat"
            name = value
        "#,
            options(&[("bad.cpat", "///bad grammar///")])
        ),
        "Failed loading expression from: \"bad.cpat\". Error details: \"[Invalid Grammar] Error \
         at line 1, column 1. Hint: syntax, use params, import, @, @\""
    );
}

#[test]
fn errors_through_a_chain_of_imports_name_every_grammar() {
    assert_eq!(
        message(
            "import { a } from \"a\"\nx = a",
            options(&[
                ("a", "import { b } from \"b\"\na = b"),
                ("b", "b = missing")
            ])
        ),
        "Failed loading expression from: \"a\". Error details: \"Failed loading expression \
         from: \"b\". Error details: \"[Invalid Grammar] unknown pattern `missing` used in `b`; \
         define it in this grammar, import it, or pass it as a param.\"\""
    );
}

#[test]
fn the_resolvers_error_is_the_builds() {
    // v12 throws the resolver's error as it is, and an enclosing import
    // wraps it like any other.
    let error = Grammar::parse_string(
        "import { value } from \"nope.cpat\"\nname = value",
        options(&[]),
    )
    .unwrap_err();
    assert_eq!(
        error,
        GrammarError::ImportResolver {
            resource: "nope.cpat".into(),
            message: "No grammar is held for \"nope.cpat\"; add it with `InMemoryImports::with`."
                .into(),
        }
    );
    assert_eq!(
        message(
            "import { a } from \"a\"\nx = a",
            options(&[("a", "import { b } from \"b\"\na = b")])
        ),
        "Failed loading expression from: \"a\". Error details: \"No grammar is held for \"b\"; \
         add it with `InMemoryImports::with`.\""
    );
}

#[test]
fn deviation_a_grammar_that_imports_itself_is_an_error() {
    // v12 recurses until the JS stack overflows and wraps the RangeError
    // once per level; here the cycle is found by resource name.
    let error = Grammar::parse_string(
        "import { a } from \"a\"\nx = a",
        options(&[("a", "import { a } from \"a\"\na = \"A\"")]),
    )
    .unwrap_err();
    assert_eq!(
        error,
        GrammarError::ImportFailed {
            resource: "a".into(),
            details: Box::new(GrammarError::ImportCycle {
                cycle: vec!["a".into(), "a".into()]
            }),
        }
    );
    assert_eq!(
        message(
            "import { b } from \"b\"\nx = b",
            options(&[
                ("b", "import { c } from \"c\"\nb = c"),
                ("c", "import { b } from \"b\"\nc = \"C\""),
            ])
        ),
        "Failed loading expression from: \"b\". Error details: \"Failed loading expression \
         from: \"c\". Error details: \"[Invalid Grammar] import cycle `b` -> `c` -> `b`: a \
         grammar can't import itself, even through others; move what they share into a grammar \
         both import.\"\""
    );
}

#[test]
fn a_grammar_imported_twice_along_different_paths_is_no_cycle() {
    let mut patterns = build(
        "import { b } from \"b\"\nimport { c } from \"c\"\nx = b + c",
        options(&[
            ("b", "import { d } from \"d\"\nb = d + \"b\""),
            ("c", "import { d } from \"d\"\nc = d + \"c\""),
            ("d", "d = \"d\""),
        ]),
    );
    assert_eq!(value(&mut patterns, "x", "dbdc").as_deref(), Some("dbdc"));
}

#[test]
fn deviation_imports_nested_too_deeply_are_an_error() {
    // A resolver that names one grammar anew on each import hides the cycle
    // from the name check; the depth limit still ends it.
    let counter = Rc::new(RefCell::new(0));
    let resolver = move |_: &str, _: Option<&str>| {
        *counter.borrow_mut() += 1;
        Ok(GrammarFile {
            resource: format!("loop-{}", counter.borrow()),
            expression: "import { a } from \"loop\"\na = \"A\"".to_string(),
        })
    };
    let error = Grammar::parse_string(
        "import { a } from \"loop\"\nx = a",
        GrammarOptions::default().with_resolver(resolver),
    )
    .unwrap_err();
    let mut depth = 0;
    let mut innermost = &error;
    while let GrammarError::ImportFailed { details, .. } = innermost {
        depth += 1;
        innermost = details;
    }
    assert_eq!(
        *innermost,
        GrammarError::ImportsTooDeep {
            resource: "loop-65".into()
        }
    );
    assert_eq!(depth, 64);
    assert_eq!(
        innermost.to_string(),
        "[Invalid Grammar] imports nest too deeply at \"loop-65\"; does the resolver give one \
         grammar a new name each time it is imported?"
    );
}

// ─── Fix round 1 regressions ────────────────────────────────────────────

#[test]
fn a_with_params_block_continues_the_importers_anonymous_count() {
    // Checked against v12: the root is `anonymous-pattern-7`. A block with
    // a count of its own would name it `anonymous-pattern-1`.
    let mut patterns = build(
        "import { v } from \"p\" with params { val = ((\"a\" | \"b\") + \"c\") }\nx = v",
        options(&[("p", "use params { val }\nv = (val + \"!\" | \"z\")")]),
    );
    let result = patterns["x"].exec("ac!");
    let tree = result.tree().expect("`x` matches");
    assert_eq!(tree.root().name(), "anonymous-pattern-7");
}

#[test]
fn an_import_is_found_before_a_param_of_the_same_name() {
    // v12 `_getPattern`: rules, then imports, then params.
    let param = build("a = \"P\"", GrammarOptions::default())["a"].clone();
    let mut patterns = build(
        "import { a } from \"a\"\nx = a",
        with_params(&[("a", "a = \"A\"")], vec![param]),
    );
    assert_eq!(value(&mut patterns, "x", "A").as_deref(), Some("A"));
    assert_eq!(value(&mut patterns, "x", "P"), None);
}

#[test]
fn a_resolver_may_answer_different_grammars_under_one_name() {
    // v12's own tests answer every import as "r1"; that is no cycle.
    let files = [
        ("a.cpat", "a = \"A\""),
        ("b.cpat", "import { a } from \"a.cpat\"\nb = a + \"B\""),
    ];
    let resolver = move |resource: &str, _: Option<&str>| {
        let (_, expression) = files
            .iter()
            .find(|(name, _)| *name == resource)
            .ok_or_else(|| format!("no {resource}"))?;
        Ok(GrammarFile {
            resource: "same".to_string(),
            expression: expression.to_string(),
        })
    };
    let mut patterns = build(
        "import { b } from \"b.cpat\"\nx = b",
        GrammarOptions::default().with_resolver(resolver),
    );
    assert_eq!(value(&mut patterns, "x", "AB").as_deref(), Some("AB"));
}

#[test]
fn a_grammar_may_import_from_the_resource_its_own_text_is_named_for() {
    let mut patterns = build(
        "import { a } from \"a.cpat\"\nx = a",
        GrammarOptions {
            origin_resource: Some("a.cpat".into()),
            ..options(&[("a.cpat", "a = \"A\"")])
        },
    );
    assert_eq!(value(&mut patterns, "x", "A").as_deref(), Some("A"));
}

#[test]
fn deviation_a_grammar_under_the_empty_name_that_imports_itself_is_an_error() {
    // v12 overflows its stack and wraps the RangeError once per level; with
    // no name to compare, the cycle is found by the grammar's text.
    let error = Grammar::parse_string(
        "import { a } from \"\"\nx = a",
        options(&[("", "import { a } from \"\"\na = \"A\"")]),
    )
    .unwrap_err();
    assert_eq!(
        error,
        GrammarError::ImportFailed {
            resource: "".into(),
            details: Box::new(GrammarError::ImportCycle {
                cycle: vec!["".into(), "".into()]
            }),
        }
    );
}

#[test]
fn deviation_unnamed_imports_nested_too_deeply_are_an_error() {
    // A resolver that answers under the empty name with new text each time
    // escapes the cycle check; every level still counts toward the depth.
    let counter = Rc::new(RefCell::new(0));
    let resolver = move |_: &str, _: Option<&str>| {
        *counter.borrow_mut() += 1;
        Ok(GrammarFile {
            resource: String::new(),
            expression: format!(
                "import {{ a }} from \"loop\"\na = \"A\"\nn{} = \"n\"",
                counter.borrow()
            ),
        })
    };
    let error = Grammar::parse_string(
        "import { a } from \"loop\"\nx = a",
        GrammarOptions::default().with_resolver(resolver),
    )
    .unwrap_err();
    let mut depth = 0;
    let mut innermost = &error;
    while let GrammarError::ImportFailed { details, .. } = innermost {
        depth += 1;
        innermost = details;
    }
    assert_eq!(
        *innermost,
        GrammarError::ImportsTooDeep {
            resource: "".into()
        }
    );
    assert_eq!(depth, 64);
}

#[test]
fn imports_of_lists_each_import_statements_resource_in_order() {
    // What an asynchronous host fetches before the build: the resources the
    // build will ask its resolver for first, one per statement.
    let text = r#"
        use params { p }
        import { a } from "first.cpat"
        import { b, c as d } from "second.cpat" with params {
            x = "x"
        }
        import { e } from "first.cpat"
        rule = a + b
    "#;
    assert_eq!(
        Grammar::imports_of(text),
        Some(vec![
            "first.cpat".to_string(),
            "second.cpat".to_string(),
            "first.cpat".to_string(),
        ])
    );
    assert_eq!(Grammar::imports_of(r#"x = "a""#), Some(Vec::new()));
}

#[test]
fn imports_of_is_none_for_text_whose_build_fails_before_importing() {
    assert_eq!(Grammar::imports_of("//"), None);
    assert_eq!(
        Grammar::imports_of(
            r#"import { a } from "f"
            x = "a" <|> "b""#
        ),
        None
    );
}
