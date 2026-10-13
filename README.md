# cpat-rs

[clarity-pattern-parser](https://github.com/jaredjbarnes/clarity-pattern-parser) in Rust, with a
WebAssembly build for JavaScript.

| Crate | What it is |
|---|---|
| `crates/cpat-core` | The parser: patterns, cursor, read-only tree. No platform code. |
| `crates/cpat-wasm` | wasm-bindgen layer plus `js/tree.mjs`, a JS view over the parse result. |

## Quick start

```rust
use cpat_core::{Literal, Pattern, Regex, Sequence};

let mut full_name: Pattern = Sequence::new("full-name", vec![
    Literal::new("first-name", "John")?.into(),
    Regex::new("space", r"\s+")?.into(),
    Literal::new("last-name", "Doe")?.into(),
])?.into();

let result = full_name.exec("John Doe");
assert_eq!(result.tree().unwrap().root().value(), "John Doe");
```

## What's in the box (milestone 11)

`Literal`, `Regex`, `Sequence`, `Choice`, `Repeat` (unbounded or bounded, with an optional
divider and `trim_divider`), `Optional`, `Not`, `Cut`, `Reference` and `Context` for recursive
grammars, `RightAssociated`, `TakeUntil`, `Block`, and `Custom` for your own patterns.

`Choice` is ordered (the first alternative that matches wins) unless some alternatives are
operators: sequences that start or end with a reference to the choice itself (`expr "+" expr`,
`"-" expr`, `expr "!"`). Then it parses by precedence, which is declaration order (earlier binds
tighter), and builds `NodeKind::Expression` nodes named after the operators. Infix operators
associate left unless wrapped in `RightAssociated`. A choice of nothing but literals (directly
or through references) is decided with a first-character index: the same result and errors as
trying each literal in order, but only the winner is parsed (`ChoiceMode::LiteralSet`;
`Choice::ordered` builds the plain loop).

A reference that names nothing comes back from `exec` as `ParseResult::pattern_error` (or from
`Pattern::check()`), never a panic. Input nested deeper than `Limits::max_depth` (1024
composites by default; `exec_with` raises it) stops the parse with `ParseResult::too_deep`
(`tooDeep` in JS) instead of overflowing the stack.

`TakeUntil` takes text, a grapheme at a time, up to where its terminator matches (or to the end).
`Block` matches literal delimiters by counting them (`{ { } }` is one block), then parses its
content only between them; with no content pattern, the inner text becomes a `block-content`
node named `<name>-content`. A block whose content refers to the block itself nests.

`exec_recording` keeps the parse's history, as v12's `exec(text, true)` does: every error and
match in order (`errors()`, `matches()`, `records()`), each with a `PatternPath`, the child
indices from the root to the pattern instance that produced it; `Pattern::at_path` replays one.
Every parse keeps the furthest matches (`leaf_matches()`), with their paths.

Introspection, for autocomplete and error messages, works without parent pointers: ask about a
place by its path from a root. `tokens()`/`patterns()` say what can start a pattern;
`next_tokens(&path)`/`next_patterns(&path)` what may follow the instance at `path`;
`tokens_after(child)`/`patterns_after(child)` what may follow one of its children. Answers that
name patterns are `Handle`s (path, pattern, name), so they can be asked about in turn; references
are followed as a parse follows them. `find` returns the first match with its path, `is_equal`
compares structure, `clone_named` renames a clone that keeps its id, and `Regex`, `TakeUntil` and
`Custom` take suggested tokens (`Tokenizable::set_tokens`). A path is followed only as deep as a
parse under the same `Limits` can make one (`Introspection::with_limits` for `exec_with`'s paths). `generate_error_message(&pattern,
&result)` prints v12's message character for character (`Error at line 1, column 6. Hint: Doe`).

`AutoComplete` suggests what could come next in a text, as v12's does: build it with a pattern
and `AutoCompleteOptions` (`greedy_pattern_names` join a pattern's suggestions with what may
follow it, `custom_tokens` add suggestions by pattern name, `disable_dedupe` keeps duplicates),
then `suggest_for(text)` returns a `Suggestion`: whether the text is complete, the options (each
a `text` to insert at `start_index`, a byte offset, with the tokens it is made of and the path
of the pattern each came from), where the text stops making sense (`error`, `error_at_index`),
and the tree of what matched. Options are sorted as JS's `localeCompare` sorts them
(`locale_compare`).

```rust
use cpat_core::{AutoComplete, AutoCompleteOptions, Choice, Literal, Pattern, Sequence};

let name: Pattern = Sequence::new("name", vec![
    Literal::new("john", "John")?.into(),
    Literal::new("space", " ")?.into(),
    Choice::new("last-name", vec![
        Literal::new("smith", "Smith")?.into(),
        Literal::new("doe", "Doe")?.into(),
    ])?.into(),
])?.into();

let mut auto_complete = AutoComplete::new(name, AutoCompleteOptions::default())?;
let suggestion = auto_complete.suggest_for("John Smi");
assert_eq!(suggestion.options[0].text, "th");
assert_eq!(suggestion.options[0].start_index, 8);
```

`Grammar::parse_string(text, GrammarOptions::default())` builds a `.cpat` grammar: every rule
comes back by name in `Patterns`, each a `Context` with every rule in scope, so a rule runs on
its own. The grammar language is v12's: literals with escapes, `/regex/`, `a + b` (and `a ~/ b`,
which inserts a cut), `a | b`, `(p)+`, `(p)*`, `(p, divider trim){m,n}`, `!p`, `p?`, `p right`,
`?->| p`, `["{"] p ["}"]` and `["{"] ... ["}"]`, groups, aliases and exports. Mistakes are a
`GrammarError` with v12's message: a syntax error with its line, column and hint, a removed `<|>`,
a name no rule defines, rules that only rename each other in a loop, and a literal an earlier,
shorter one hides (`"=" | "=="`). Text nested deeper than 10 levels of parentheses, or more
than 256 operators deep, is refused too (`GrammarError::NestedTooDeep`), where v12 overflows or
runs out of memory. The hand-written cpat grammar itself is `cpat_grammar()`.

```rust
use cpat_core::{Grammar, GrammarOptions};

let mut patterns = Grammar::parse_string(r#"
    number = /[0-9]+/
    mul = expr + "*" + expr
    add = expr + "+" + expr
    expr = mul | add | number
"#, GrammarOptions::default())?;

let result = patterns["expr"].exec("1+2*3");
assert_eq!(result.tree().unwrap().root().name(), "add");
```

Grammars import grammars: `import { a, b as c } from "lib.cpat" with params { x = y }` loads
`lib.cpat` through the `ImportResolver` in `GrammarOptions::resolver` (synchronously: a host that
fetches fetches first, then answers from memory; `InMemoryImports` does that, and a closure is a
resolver too). `use params { x, y = default }` declares what a grammar takes; `params` in the
options supply it. `Grammar::import(resource, options)` builds a grammar the resolver finds.
Decorators above a rule hand its pattern to a function: `@tokens(["a", "b"])` is built in, and
`GrammarOptions::decorators` registers your own (`Fn(Pattern, Option<&Json>) -> Result<Pattern,
String>`), which may replace the pattern, with a `Custom` one for instance. Errors are v12's,
word for word (`Failed loading expression from: "lib.cpat". Error details: "…"`).

`patterns(text)` (or `patterns!`, which formats like `format!` when given arguments) returns the
rules keyed by camelCase name, as v12's template does; `patterns_with(options)` returns a builder
that uses those options every time.

```rust
use cpat_core::{Grammar, GrammarOptions, InMemoryImports};

let imports = InMemoryImports::new().with("names.cpat", r#"first = "John""#);
let mut patterns = Grammar::parse_string(r#"
    import { first } from "names.cpat"
    @tokens([" "])
    space = /\s+/
    full = first + space + "Doe"
"#, GrammarOptions::default().with_resolver(imports))?;

assert!(patterns["full"].test("John Doe"));
assert_eq!(patterns["space"].tokens(), [" "]);

let rules = cpat_core::patterns!(r#"full-name = "John Doe""#)?;
assert!(rules["fullName"].clone().test("John Doe"));
```

Still to come: autocomplete and grammars in JS.

```rust
use cpat_core::{Block, Choice, Literal, Pattern, Reference, Regex, Sequence};

// expr = "(" expr ")" | "x"
let mut expr: Pattern = Choice::new("expr", vec![
    Sequence::new("group", vec![
        Literal::new("open", "(")?.into(),
        Reference::new("expr").into(),
        Literal::new("close", ")")?.into(),
    ])?.into(),
    Literal::new("x", "x")?.into(),
])?.into();

assert!(expr.test("((x))"));

// expr = expr "*" expr | expr "+" expr | /[0-9]+/   ("*" binds tighter)
let e = || Reference::new("expr").into();
let mut arithmetic: Pattern = Choice::new("expr", vec![
    Sequence::new("mul", vec![e(), Literal::new("times", "*")?.into(), e()])?.into(),
    Sequence::new("add", vec![e(), Literal::new("plus", "+")?.into(), e()])?.into(),
    Regex::new("num", "[0-9]+")?.into(),
])?.into();

let result = arithmetic.exec("1+2*3");
assert_eq!(result.tree().unwrap().root().name(), "add");

// block = "{" block "}", where "{}" is the base case: adjacent delimiters always match
let mut block: Pattern = Block::new(
    "block",
    Literal::new("open", "{")?,
    Some(Reference::new("block").into()),
    Literal::new("close", "}")?,
).into();

assert!(block.test("{{{}}}"));
```

## Working on it

```sh
cargo test --workspace                       # Rust tests
cd crates/cpat-wasm
wasm-pack build --target nodejs --out-dir pkg-node
node --test js/*.test.mjs                     # JS smoke tests against the wasm build
```

**Positions are byte offsets in Rust and UTF-16 offsets in JS.** The wasm layer converts at the
boundary; never mix the two.

Why it's built this way: [docs/DESIGN.md](docs/DESIGN.md).
