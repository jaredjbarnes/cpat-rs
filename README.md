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

## What's in the box (milestone 5)

`Literal`, `Regex`, `Sequence`, `Choice`, `Repeat` (unbounded or bounded, with an optional
divider and `trim_divider`), `Optional`, `Not`, `Cut`, `Reference` and `Context` for recursive
grammars, `RightAssociated`, and `Custom` for your own patterns.

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
Still to come: `TakeUntil`, `Block`, the `.cpat` grammar,
decorators, intellisense.

```rust
use cpat_core::{Choice, Literal, Pattern, Reference, Regex, Sequence};

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
