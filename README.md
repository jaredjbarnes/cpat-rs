# cpat-rs

[clarity-pattern-parser](https://github.com/jaredjbarnes/clarity-pattern-parser) in Rust, with a
WebAssembly build for JavaScript.

| Crate | What it is |
|---|---|
| `crates/cpat-core` | The parser: patterns, cursor, read-only tree. No platform code. |
| `crates/cpat-wasm` | wasm-bindgen layer plus clarity-pattern-parser's JS API over it: `js/index.mjs` for Node, `js/web.mjs` for browsers and bundlers. |
| `conformance` | The differential conformance harness (TS v12 vs native vs wasm), its corpus, and the speed report. |
| `bench` | The benchmark: TS v12 vs native vs wasm on verified-identical workloads (grammar builds, 1 MB documents, micro cases, AutoComplete), with the wasm cost split and memory. `node bench/run.mjs`; see [bench/README.md](bench/README.md) and the last [results](bench/RESULTS.md). |

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

## What's in the box (milestone 16, gaps 1-1 to 3-1)

`Literal`, `Regex`, `Sequence`, `Choice`, `Repeat` (unbounded or bounded, with an optional
divider and `trim_divider`), `Optional`, `Not`, `Cut`, `Reference` and `Context` for recursive
grammars, `RightAssociated`, `TakeUntil`, `Block`, and `Custom` for your own patterns (a
`CustomParse` whose own code fails stops the parse with `Cursor::halt_custom`, and `exec`
returns `PatternError::CustomFailed`).

A `Regex` source is JavaScript regex syntax, as v12 compiles it (the `u` flag): it is parsed as
JS reads it, so what JS rejects (`[\d-z]`, `\p{letter}`, `a{,3}`) is a `PatternError`, and `.`,
`\s`, `\d`, `\w`, `\b`, `\cX`, `(?ims-ims:…)` and backreferences mean what they mean in JS. The
few sources JS accepts that no Rust engine matches the same way (a backreference to a group a
repetition resets) are refused when built; DESIGN.md §6 has the full mapping. A compiled regex
may take 128 MiB, so `\p{L}{1,1000}` and `.{0,65535}` build, and groups nest 256 deep; a
program past that size (`a{4294967295}`) or a translation nested past what the compiler's stack
survives (88 nested quantified groups) is a build error rather than an out-of-memory or
stack-overflow abort (DESIGN.md §6, "Compile limits").

Everything below behaves as clarity-pattern-parser v12 does, checked by running both on 156 364
cases from real test suites and a generated non-ASCII corpus
([Conformance with v12](#conformance-with-v12)); the few differences are listed and explained in
`conformance/allowlist.json` and DESIGN.md §23. `exec` matches only when every code point of the
text is in the tree, as v12's `ast.value === text` does: a literal that matches the base of a
letter and a combining mark leaves the mark unmatched, and that is no match.

`Introspection::choice_roles` says what each of a choice's classified children is
(`ChoiceRole::Atom`, `Prefix`, `Postfix`, `Infix`), classifying a choice that waits for its first
parse as that parse would. `Repeat::requested_min`/`requested_max` keep the bounds as given
(v12's `Repeat.min`/`max`) while `min`/`max` are what the parse uses (a bounded `max` of 0
means `min`), and
`grapheme_starts(text)` lists where each character starts, as the parser positions on them.

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
follow it, up to 32 joins deep and 10 000 places per suggestion, DESIGN.md §17, `custom_tokens` add suggestions by pattern name, `disable_dedupe` keeps duplicates),
then `suggest_for(text)` returns a `Suggestion`: whether the text is complete, the options (each
a `text` to insert at `start_index`, a byte offset, with the tokens it is made of and the path
of the pattern each came from), where the text stops making sense (`error`, `error_at_index`),
and the tree of what matched. Options are sorted as JS's `localeCompare` sorts them
(`locale_compare`): exactly for ASCII, every letter in U+00C0–U+017F and the combining marks
they decompose to (in canonical order, as ICU normalizes them), and any other
character after those letters, by code point.

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
shorter one hides (`"=" | "=="`). Text nested deeper than 10 levels of parentheses (9 of
divided repeats, 7 of blocks), or more than 256 operators deep, is refused too (`GrammarError::NestedTooDeep`), where v12 overflows or
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
String>`), which may replace the pattern, with a `Custom` one for instance. The argument is
what v12's `JSON.parse` gives: an object's keys in the order written, each number the nearest
double (integers that fit 64 bits stay exact in Rust; JS gets v12's doubles); a lone
surrogate escape such as `"\ud800"` reads as U+FFFD, as it does in a cpat literal. Errors are v12's,
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

A parse gives a read-only `Tree` whose values are slices of the input. It walks and searches
without copying: on a `Node`, `walk_up`, `walk_down` and `walk_breadth_first` (a visitor
returning `false` stops), `find`/`find_all` and their `_breadth_first` forms, `flatten` (the
leaves) and `to_json(space)`, v12's JSON key for key (`id`, `type`, `name`, `value`,
`startIndex`, `endIndex`, `children`; ids are strings).

To change a tree, `tree.to_mut()` copies it into an `EditableTree` (leaves still borrow the
input) with v12's whole `Node` API, called with a `NodeRef` handle: `id`, `type_name`, `name`,
`value`, `first_index`/`last_index`/`start_index`/`end_index`, `parent`, `children`,
`has_children`/`is_leaf`; `remove_child`, `find_child_index`, `splice_children`,
`remove_all_children`, `replace_child`, `replace_with`, `insert_before`, `append_child`,
`append`, `remove`; `next_sibling`, `previous_sibling`, `find_root`, `find_ancestor`; the walks
and finds above (a visitor may edit the tree); `transform` with `Visitors` by node name,
`compact`, `clone_node`, `normalize` (renumber positions from the values' lengths, in bytes),
`is_equal`, `to_string`, `to_cycle_free_object` and `to_json`. `create_node`,
`create_value_node` and `new_node` make nodes; `ast::compact(tree, node, names)` and
`ast::remove(tree, node, names)` act on every node with one of those names, breadth first.

```rust
use cpat_core::ast::{compact, remove};
use cpat_core::{EditableTree, Visitors};

let mut patterns = cpat_core::patterns!(r#"
    space = /\s+/
    number = /\d+/
    group = "(" + space? + expr + space? + ")"
    add = expr + space? + "+" + space? + expr
    expr = add | group | number
"#)?;
let result = patterns.get_mut("expr").unwrap().exec("(1 + 2) + 3");
let mut tree = result.tree().unwrap().to_mut();
let root = tree.root().unwrap();

compact(&mut tree, root, &["group"]);
remove(&mut tree, root, &["space"]);
let mut visitors = Visitors::new().on("number", |t: &mut EditableTree<'_>, n| {
    let value = t.to_string(n);
    t.create_value_node("number", "num", value)
});
let root = tree.transform(root, &mut visitors);

assert_eq!(tree.value(root), "(1 + 2)+3");
assert_eq!(tree.children(root).len(), 3);
```

`Selector` and `Query` select from an editable tree with CSS-like selectors, as v12's do:
names, `*`, `[property op 'text']` (`=`, `!=`, `^=`, `$=`, `*=`, `>=`, `<=`, `>`, `<` on `name`,
`value`, `type`, `endIndex` and v12's other node properties, compared as JS compares them, so
`[endIndex>=35]` is numeric), `name[…]`, the combinators `a b`, `a > b`, `a + b`, `a ~ b`, and
`a, b`. `Selector` has `find`, `filter`, `not` and `parents`. `Query` (v12's `$`) narrows with
`find`, `filter`, `not`, `parent`, `parents`, `first`, `last`, `slice` and `end`, and edits every
selected node with `append`, `prepend`, `after`, `before`, `replace_with`, `set_value`,
`compact`, `remove` and `normalize`. A selector that doesn't parse is a `SelectorError` with
v12's message, never a panic.

```rust
use cpat_core::{EditableTree, Query};

let mut tree = EditableTree::new();
let one = tree.create_value_node("node", "child", "1");
let two = tree.create_value_node("node", "child", "2");
let root = tree.create_node("node", "parent", &[one, two]);

Query::select(&tree, root, "parent > [value='2']")?.set_value(&mut tree, "3");
assert_eq!(tree.to_string(root), "13");
assert!(Query::select(&tree, root, "parent >").is_err());
```


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

From JavaScript, all of it is clarity-pattern-parser v12's API, in Node (`js/index.mjs` over
wasm-pack's `nodejs` build) and in browsers and bundlers (`js/web.mjs` over its `web` build,
after `await init()`); see [The JS API](#the-js-api).

## The JS API

`crates/cpat-wasm/js/index.mjs` exports what clarity-pattern-parser v12's `src/index.ts` exports,
with the same shapes, so code written against v12 mostly runs unchanged: the pattern classes
(`Literal`, `Regex`, `Sequence`, `Expression`, `Optional`, `Not`, `Cut`, `Repeat`, `Reference`,
`Context`, `RightAssociated`, `TakeUntil`, `Block`) with v12's constructors
(`new Repeat(name, pattern, { min, max, divider, trimDivider })`), `Grammar` (`parseString`,
async `parse` and `import`, `resolveImport`/`resolveImportSync`, `params` as JS patterns, JS
`decorators`), `patterns` and `createPatternsTemplate`, `Node`, `compact`, `remove`, `Query`,
`$`, `Selector`, `Cursor`, `ParseError`, `AutoComplete` (with its static `suggestFor` and
`suggestForWithCursor`), `generateErrorMessage`, `isTokenizablePattern` and `grammar`; and one
class v12 doesn't have, `CustomPattern`, for patterns written in JS.

The same API comes from two entries, one per wasm-pack target (build in `crates/cpat-wasm`):

| Where | Build | Entry | Loading |
|---|---|---|---|
| Node | `wasm-pack build --target nodejs --release --out-dir pkg-node` | `js/index.mjs` | Ready on import (the wasm loads synchronously). |
| Browsers and bundlers | `wasm-pack build --target web --release --out-dir pkg-web` | `js/web.mjs` | `await init()` before using it. No Node built-ins anywhere it reaches. |

```js
import init, { patterns } from "./crates/cpat-wasm/js/web.mjs";

await init(); // fetches pkg-web/cpat_wasm_bg.wasm, found next to the glue via import.meta.url
// or: await init(fetch(wasmUrl)) / await init(wasmBytes) / initSync(wasmBytes or a WebAssembly.Module)
const { num } = patterns`num = /\\d+/`;
```

Bundlers that understand `new URL("…", import.meta.url)` (Vite, webpack 5, esbuild with a file
loader, Parcel) copy the `.wasm` for `init()`; otherwise pass `init` the URL you serve it from.
Until `init` resolves, every pattern constructor and parse throws "wasm isn't loaded", and
`grammar` is `undefined`. Use one entry per program: loading the other one too throws, because
patterns from two wasm instances can't be mixed.

```js
import { patterns, Grammar, $ } from "./crates/cpat-wasm/js/index.mjs";

const { fullName } = patterns`
  first-name = "John"
  space = /\\s+/
  last-name = "Doe"
  full-name = first-name + space + last-name
`;
const { ast, cursor } = fullName.exec("John  Doe");
ast.find(n => n.name === "space").remove();          // v12's editable Node
$(ast, "last-name").setValue("Smith");
console.log(ast.value, cursor.hasError);              // "JohnSmith" false

const grammar = await Grammar.parse(`import { name } from "names.cpat"\nhi = "hi " + name`, {
  resolveImport: async resource => ({ resource, expression: await load(resource) }),
});
```

- `exec(text, record?)` returns `{ ast, cursor }`. `ast` is a `Node` made from the flat result
  (`tree`, a read-only view over `Uint32Array`s) the first time it is read; the view's `toNode()`
  does the same for any subtree. Positions are UTF-16 offsets: `firstIndex`/`lastIndex`
  inclusive and `startIndex`/`endIndex` half-open, as in v12. The cursor reports `error`,
  `furthestError`, `leafMatch(es)` and, after a recording exec, `errors`, `records` and
  `rootMatch`, each naming its pattern.
- A pattern's `children` are places in it: a child knows its `parent` and what follows it
  (`getNextTokens`), but is a view, not a separate pattern. Composite constructors copy the
  patterns they are given, as v12's clone them, so changing one afterwards changes nothing in
  the composite. `setTokens` works on a pattern you hold, not on a child place.
- `Grammar.parse` and `Grammar.import` fetch every import with your `resolveImport` first (in
  the order v12 asks, from the same origins), then build synchronously. Grammar errors are
  thrown as `Error`s with v12's message, character for character.
- `new CustomPattern(name, (text, pos) => end | null)` is a pattern whose matching is your
  function, asked once per attempt with UTF-16 positions; its node is the text it took. Use it
  directly, or return one from a grammar decorator to put it in place of a rule's pattern:

  ```js
  const word = pattern => new CustomPattern(pattern.name, (text, pos) => wordEnd(text, pos));
  const { items } = createPatternsTemplate({ decorators: { word } })`
    @word
    word = /never-used/
    items = word + "," + word
  `;
  ```

  A decorator may also change the pattern in place (`pattern.setTokens([...])`, as v12's do);
  what it returns that isn't a pattern is ignored. A function that throws, or answers anything
  but `null` or an end in the text, stops the parse: `exec` throws `custom pattern \`name\`
  failed: …`.
- The classes have v12's members: `Repeat`'s `min`, `max` (`Infinity` when unbounded), `pattern`
  and `options`; `Expression`'s `atomPatterns`, `prefixPatterns`, `postfixPatterns`,
  `infixPatterns` (places among its `children`) and `originalPatterns`; and a `Cursor` that steps
  a grapheme at a time (`currentChar`, `next`, `previous`, `getCharStartIndex`, …) with the
  parser's own boundaries, plus v12's `recordErrorAt`/`recordMatch`/choice-scope methods for
  code that drives a cursor by hand (a cursor that matched nothing has v12's one empty leaf
  match). `Expression.build()` throws v12's message for an alternative that is a reference naming
  nothing; its accessors are filled before `build()`, where v12's are empty (DESIGN.md §22).
  `js/index.test.mjs` checks every public member of v12's
  `Cursor` and pattern classes; the two not here are listed in DESIGN.md §22.
- `AutoComplete.suggestFor(text)` answers v12's `Suggestion`: `options` (`text`, `startIndex` in
  UTF-16, and `suggestionSequence` segments whose `pattern` is a place with `name` and `type`),
  `error`, `errorAtIndex`, `isComplete`, `ast` and a recording `cursor` with the parse's errors
  and matches; `suggestForWithCursor(cursor)` reports into the cursor you pass, as v12's does.
- Differences from v12 are those of the Rust core (DESIGN.md lists them), plus: an error from an
  ordinary (not recording) parse names its pattern by `name` and `id` only; a reference that
  names nothing throws when `exec` starts, not when the parse reaches it; input nested too deep
  is `cursor.tooDeep`, not a `RangeError`.

## Conformance with v12

`node conformance/run.mjs` is the parity check. It builds the release native engine
(`conformance/native`) and the wasm package, then runs the corpus through TS v12 (under tsx,
against v12's `src`; set `CPAT_V12` if the checkout isn't at `../clarity-pattern-parser-v12`),
cpat-rs native and cpat-wasm, and compares both cpat engines with v12: grammar builds (or the
error message), match or no match, every tree (names, types, UTF-16 ranges, values), the error
and furthest error, `generateErrorMessage`, and AutoComplete's options, `isComplete` and
`errorAtIndex`. It prints a summary and the differences grouped by kind, writes
`target/conformance/report.md`, and exits 0 only when every difference is explained by
`conformance/allowlist.json` (each entry cites the decision behind it in DESIGN.md or
MIGRATION-v12.md) and no entry is stale. Last run: 156 364 cases (82 from grammars with
imports, 56 with params, 9 with decorators; 15 863 from the generated non-ASCII corpus), 0
unexplained differences.

- `--no-build` reuses the existing builds; `--examples N` prints more examples per group.
- `--self-test` shifts one native tree range on purpose (`--perturb`) and checks that the run
  fails with exactly that unexplained difference.
- The corpus (`conformance/corpus/*.json`) is what real tests ran: every input agent-harness's
  vitest suite runs through its 11 `.cpat` grammars, and every input v12's
  `typescript.test.ts`, `Grammar.test.ts`, `Cut.test.ts` and `patterns.test.ts` run through
  their grammars. A grammar built with imports, params or custom decorators is recorded with
  them and replayed: imports are answered from the resolver's recorded answers, and each custom
  decorator is replaced by one that records what it received, compared across engines. The
  run adds a truncation at every token boundary (also run through AutoComplete) and a few
  single-character deletions of each input. `node conformance/capture/capture.mjs --scratch
  <dir>` recaptures it, configuring both test runs in `<dir>` only (set `AGENT_HARNESS` if
  agent-harness's `packages/harness` isn't at `../agent-harness/packages/harness`).
- `corpus/unicode.json` is generated (`node conformance/capture/unicode.mjs`): captured inputs
  with every space swapped for `\r`, `\r\n`, U+0085, U+2028/2029, U+FEFF and other unusual
  whitespace, non-ASCII words inserted (combining marks, skin tones, CJK, letters outside the
  BMP) and surrogate pairs at the end, plus a grammar of regex-heavy rules run on the same
  characters and a grammar of literals and a Pratt expression with a joiner (combining mark,
  skin tone, ZWJ, U+FE0F) after each character. Every generated case runs both `exec` and
  AutoComplete on the whole input.
- `node --test conformance/test/*.test.mjs` tests the harness itself, including that dropping
  any compared field from the normalization or the comparison fails a test.

`node conformance/bench.mjs --out <dir>` is the speed report, not a test (nothing is asserted):
grammar build, `exec` throughput and AutoComplete for every corpus grammar in TS v12, native
(release) and wasm (release), with warm-up and 15 timed runs each (`--runs`, `--warmup`), written
to `<dir>/bench.md` (median and p90 per grammar, speed-ups over v12) and `<dir>/bench.json`.
DESIGN.md §23 summarizes the last one: native parses 3–5× faster than v12; AutoComplete in wasm
is the slow spot.

## Working on it

```sh
cargo test --workspace                       # Rust tests
cd crates/cpat-wasm
wasm-pack build --target nodejs --release --out-dir pkg-node
wasm-pack build --target web --release --out-dir pkg-web
node --test js/*.test.mjs                     # JS tests against both wasm builds
cd ../.. && node conformance/run.mjs          # parity with TS v12 (needs v12's checkout)
```

**Positions are byte offsets in Rust and UTF-16 offsets in JS.** The wasm layer converts at the
boundary; never mix the two.

Why it's built this way: [docs/DESIGN.md](docs/DESIGN.md).
