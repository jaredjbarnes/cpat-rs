# cpat-rs design

> **Status:** decided 2026-10-11. Items 1–9 are settled. Do not re-propose an alternative
> without new evidence. Milestone 1 (cursor, tree, Literal, Regex, Sequence, ordered Choice,
> Optional, Not, Cut, Custom, wasm layer) is implemented. Milestone 2 (`Repeat`, unbounded and
> bounded, with dividers, `trim_divider` and cut restoration) is implemented; see §10.

The behavior target is clarity-pattern-parser **v12** (branch `release/v12`). Where v11 had a
bug, cpat-rs does the correct thing and v12 gets the same fix. `MIGRATION-v12.md` in that repo
lists those fixes.

## 1. Patterns own their state and are cloned

Each pattern struct keeps its per-parse scratch state (`Sequence`'s node list, for example) on
itself, and `parse` takes `&mut self`. A pattern instance is never re-entered: recursion always
reaches a fresh clone. Every pattern is `Clone`; custom patterns implement `clone_box`.

- **Rejected:** an immutable grammar with parse state on the stack. It would allow parsing in
  parallel, which we never want, and it would stop following the TS model.
- **Gain:** scratch buffers keep their capacity, so steady-state parsing doesn't allocate in them.

## 2. No parent pointers in patterns

A child can't point at a parent that is mutably borrowed while parsing. The cursor keeps the
ancestor stack instead: `(PatternId, start)` per active composite. That stack drives the
recursion guard. Start indices on the stack never decrease, so the guard scans down from the top
only while the start equals the current position. Autocomplete will snapshot the path of the
furthest leaf match. *(Ancestor stack: lands with `Reference`.)*

## 3. References resolve by name against a shared scope

`Context` pushes an `Rc<Scope>` of master definitions onto the cursor. A `Reference` looks its
name up on the scope stack, from the top down, the first time it parses. It then clones the
master copy as its own child. Each recursion level therefore gets its own instance, created
lazily. The direct API builds an implicit scope before parsing. Unknown names are build errors.
*(Lands with `Reference`.)*

## 4. Cut: only kept work commits

A choice runs its alternatives inside a scope on the cursor (`in_choice_scope`). `Cut` marks
the innermost scope. When an alternative fails after its cut, the choice fails and keeps the deep
error. Patterns that throw away a child's work (`Optional` on failure, `Not` always, repeats when
they drop an iteration, `TakeUntil` probes) restore the flag they saw, so a cut inside discarded
work commits nothing. A cut reached at the end of input still fires.

## 5. Positions

- **5a:** byte offsets into `&str`.
- **5b:** half-open. After a match, the cursor sits just past it, and end of text is
  `pos == len`. The end-of-text checks that v12 has are kept on purpose: the TS experiment showed
  they decide observable behavior (furthest error, cut). See `Sequence::finish_at_end`.
- **5c:** grapheme boundaries come from `Boundaries`. ASCII text needs no table; anything else
  uses one bit per byte.
- **5d:** JS gets UTF-16 offsets, converted at the wasm boundary.

## 6. Regex

`enum Matcher { Fast(regex), Fancy(fancy_regex) }` is chosen when the pattern is built. The
linear-time `regex` crate is used unless the source needs lookaround or backreferences. The
pattern compiles as `^(?:src)` and matches on `&text[pos..]`, so `\b` and lookbehind treat the
cursor as the start of input, the same as the JS substring. `regex_translate` rewrites JS-only
syntax (ASCII `\d \w \b`, class escapes, `[^]`, `\0`, `\/`). A source that won't compile is a
build error naming the pattern.

- **Rejected:** calling JS `RegExp` from wasm. It would make native and browser results differ
  and cost a boundary crossing per call.
- **The cost we're accepting:** wasm size. The milestone-1 release build is 1.3 MB, mostly the
  two engines' Unicode tables. Measure before trimming features.

## 7. The tree

- **7a:** `Tree` is one flat arena: `Vec<NodeData>` plus a child-id pool. Names are interned
  (`Name` is a `u32`), and values are slices of the input.
- **7b:** an editable form (`to_mut`) gets built only when something needs to edit a tree.
- **7c:** the arena only grows during a parse. Nodes from failed alternatives stay in it,
  unreachable. History stores copies (`Match`, `ParseError`), not node references.
- **7d:** JS gets flat `Uint32Array`s with a thin `Node` view class (`js/tree.mjs`), so walking
  the tree never calls into wasm.

## 8. The grammar *(next milestones)*

- The cpat grammar is written by hand with the Rust API. Later, as a test, cpat's own grammar
  will be written as a `.cpat` file and built by the builder.
- The builder walks the read-only tree and flattens nested `+`/`|` on the fly.
- Synchronous core with an `ImportResolver` trait; JS fetches imports ahead of time.
- Errors are values (`GrammarError`).
- Anonymous names are counted per grammar.
- **One `Choice` pattern** with modes `Ordered`, `LiteralSet` and `Pratt`, picked at build time
  from the classified alternatives (references resolved one level, alias cycles detected).
  There is no `Options`/`Expression` split and no greedy `<|>`.
- Shadowed literals, unknown names and alias cycles are build errors.

## 9. Decorators *(next milestones)*

- A decorator is `Fn(Pattern, Option<&Json>) -> Result<Pattern, String>`. It can wrap or replace
  a pattern, for example to inject a `Custom` one.
- Mistakes are build errors.
- JS decorators are fully supported. A JS custom pattern uses a minimal contract,
  `(text, pos) => end | null`, so each attempt is one boundary call.

## 10. Repeat *(milestone 2)*

- **One `Repeat` type, strategy chosen at construction.** `max: None` builds the `Unbounded`
  strategy (v12 `InfiniteRepeat`), `Some(max)` the `Bounded` one (v12 `FiniteRepeat`). The
  strategy is an exhaustive private enum; parsing never branches on `max`. Node kinds stay
  `InfiniteRepeat` and `FiniteRepeat`, as in v12.
- **Both strategies copy v12 line for line,** quirks included: min is clamped to 1, a bounded
  `max` of 0 means `min`, a zero-width child with no divider ends the loop with "no node, no
  error", the bounded repeat counts zero-width child slots toward `min` and decides "ended on a
  divider" by node-count parity, only the unbounded repeat records a match, and its node starts
  where the repeat started while the bounded node starts at its first kept node. Only the
  bounded repeat rewinds after a slot that returned no node; the unbounded one leaves the cursor
  where the child left it and tries the divider from there (a child can return "no node, no
  error" after moving, e.g. an inner `(a?)+`). Both stop at the end of the text after a kept
  node or divider, translated from v12's `!cursor.hasNext()` to `at_end()`, so nothing records
  an error past the end.
- **A bounded repeat has one child and one divider, not `max` clones.** v12 clones the child into
  every slot. Slots run one after another and never overlap, so a slot is just an index here, and
  `{1,1000}` doesn't build a thousand patterns. Clones share ids anyway, so nothing observable
  changes. *(Introspection that names "the child in slot i" will use the index.)*
- **A bound of any size is safe.** The slot count saturates instead of overflowing (`max` comes
  from user input; JS passes up to `u32::MAX` and wasm's `usize` is 32 bits, where `2 * 2^31`
  used to wrap to 0 slots). And a lap (child, then divider if any) that consumed nothing ends the
  walk, its remaining child slots counted as the zero-width matches they would be: from an idle
  lap every later lap is identical, so the result equals v12's, but a huge bound can't spin.
  v12 would allocate `max` clones instead; no input it handles behaves differently.
- **`last` of an error recorded "where the cursor stopped" at the end of the text** is the start of
  the last grapheme (`Cursor::last_index`), because v12's inclusive cursor can't move past it. It
  decides which error is furthest, so it is kept.
- **Deviations from v12**, each pinned by a test in `tests/repeat.rs`:
  - A bounded repeat with `trim_divider` whose first child fails throws a `TypeError` in v12 (it
    pops a node that isn't there). Here there is nothing to trim, and it fails on `min` like the
    untrimmed repeat.
  - A bounded repeat whose only matches were zero-width and whose next slot failed throws a
    `TypeError` in v12. Here the failed slot is dropped like any trailing failure: a zero-width
    success.
  - `Repeat::min()` reports the clamped minimum. v12's `Repeat` facade reports the unclamped
    option while its implementation uses the clamped one.
  - No `startedOnIndex`: patterns keep no position for others to read (§2).
  - A `Custom` pattern that returns a zero-width node gets a zero-width success instead (no
    node, cursor back where it began), which is what its contract asks for. Kept as a node, it
    would make an unbounded repeat loop forever: v12 always advances with `cursor.next()` after a
    kept node, which half-open positions don't need. No built-in makes a zero-width node, so only
    custom code can reach this; checked once, in `Custom::parse`, so no composite has to.
- **Differences from milestone 1 that show through a repeat** (found by running both on the same
  inputs; they belong to Literal and the tree, not to Repeat, and are kept):
  - On empty text v12's Literal records its error with `lastIndex` −1. Positions here are
    unsigned and half-open (§5), so it records `last` 0. A repeat's own `min` error at 0..0 then
    ties with the literal's instead of beating it, and the furthest error stays the literal's
    (`(a?, ",")+` on `""`: v12 says `r`, cpat-rs says `a`). The same holds for `!a` children and a
    `Sequence` of `!a`. The current error is the same in both.
  - A composite node's value is a slice of the text it spans (§7a); in v12 it is the join of its
    children's values. They differ when the span holds text no child covers: a literal `e`
    matching the base of `e` + combining accent skips the accent, and an unbounded repeat whose
    node starts where the repeat started can begin with text only a nodeless child consumed.
    `exec` compares the value's length with the text's, so it can match here where v12 reports
    "did not consume all the text" (`(e){1,3}` on `"ée"`, `((a?)+, ",")+` on `"aa,"`). Slices are
    a settled decision, and the slice is the honest value: it is what the node's range covers.
- **JS:** `Pattern.repeat(name, child, { min, max, divider, trimDivider })` takes a plain object,
  read through imported getters (no `js-sys`). `max: Infinity` is unbounded, as in v12; a count
  that isn't a whole number of zero or more is an error.

## Behavior fixed relative to clarity-pattern-parser v11

Each is pinned by a test here and fixed in v12:

| Fix | Test |
|---|---|
| A cut inside discarded work doesn't commit | `tests/cut.rs` |
| A cut at the end of input fires | `tests/cut.rs` |
| `!x` at the end of input holds | `tests/sequence.rs` |
| A Regex node's value equals its span | `tests/regex.rs` |
| `furthest_error` after a failed `exec` is the deepest real failure | `tests/exec.rs` |
| A trailing escaped `\$` in a regex isn't treated as an anchor | `tests/regex.rs` |
| A successful Regex clears the current error (v11's Regex didn't) | — |
