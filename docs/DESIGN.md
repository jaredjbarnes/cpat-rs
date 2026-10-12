# cpat-rs design

> **Status:** decided 2026-10-11. Items 1–9 are settled. Do not re-propose an alternative
> without new evidence. Milestone 1 (cursor, tree, Literal, Regex, Sequence, ordered Choice,
> Optional, Not, Cut, Custom, wasm layer) is implemented. Milestone 2 (`Repeat`, unbounded and
> bounded, with dividers, `trim_divider` and cut restoration) is implemented; see §10.
> Milestone 3 (`Reference`, `Context`, the ancestor stack, recursion guards, the implicit scope)
> is implemented; see §11.

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
furthest leaf match. *(Implemented in milestone 3: `Cursor::ancestors()`, see §11.)*

## 3. References resolve by name against a shared scope

`Context` pushes an `Rc<Scope>` of master definitions onto the cursor. A `Reference` looks its
name up on the scope stack, from the top down, the first time it parses. It then clones the
master copy as its own child. Each recursion level therefore gets its own instance, created
lazily. The direct API builds an implicit scope before parsing. Unknown names are build errors.
*(Implemented in milestone 3. Unknown names come back as `PatternError::UnknownReference`; see §11.)*

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
- **`last` of an error recorded "where the cursor stopped"** (a repeat's own `min` failure) is
  where v12's cursor would be, `Cursor::last_index`. At the end of the text that is the start of
  the last grapheme, because v12's inclusive cursor can't move past it. After some failures v12's
  cursor is not back where the failed pattern began: a `Literal` stays on the character that
  didn't match, a `Sequence` that ran out of text stays at the end, and a `Sequence` whose last
  child moved without a node stays there. That spot decides the repeat's error (`"ab"{2,}` on
  `"abac"` fails at `r@0..3`, not `0..2`), and through it which error is furthest, so it is kept:
  the failing pattern notes it with `Cursor::stopped_at`, and the next move forgets it.
- **A failed pattern leaves `pos` where it began.** This one is intentional: in v12 `cursor.index`
  after a failure can sit at the spots above (`(a+, "b")` on `"aa"` leaves it at 1). Nothing
  parses from there, and every parent rewinds or fails in turn, so the only observable use of the
  spot is the error `last` above, which matches. Callers driving `parse` directly see `pos` at
  the failed pattern's start.
- **`!x` at the end of the text inside a `Sequence`** is decided as v12 decides it: `x` runs on
  a throwaway empty cursor (`Not::child_matches_empty`), so nothing it records reaches the real
  parse. (Milestone 1 parsed `x` on the real cursor, so its failure became the furthest error:
  `("ab", !",")` on `"ab"` reported `,@2..2` where v12 reports none.)
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

## 11. Reference and Context *(milestone 3)*

- **The ancestor stack.** `Pattern::parse` pushes a `Frame { id, start, child }` for every
  composite (Sequence, Choice, Repeat, Optional, Not, Reference, Context) around its parse, in
  the dispatch `match`, so no composite can forget to. Leaves push nothing. `child` is reserved
  for intellisense and always 0 for now: writing it would cost every composite a store per child
  for a feature nothing reads yet. When intellisense lands, it snapshots the path of the
  furthest leaf from this stack, filling `child` then. Guards ask "how many frames with this id
  began here?"; starts never decrease going up, so the scan stops at the first frame that began
  earlier.
- **Two recursion guards, both v12's.** A `Reference` fails, with its own error at
  `(pos, pos)`, when more than one frame with its target's id began at this position (v12
  `_isBeyondRecursiveAllowance`). An ordered `Choice` skips an alternative when a frame with
  that alternative's id began at this position: v12's choice is `Expression`, whose atom path
  has this guard. It isn't optional: without it, `a = a "b" | "b"` still ends (through the
  reference guard) but records the reference's error, so the furthest error differs from v12.
  It is also what ends `a = b | "c"`, `b = a | "d"`.
- **Resolution.** On its first parse a `Reference` looks its target name up on the cursor's
  scope stack, innermost first, skipping a reference and a context that only wraps one (v12
  `_isValidPattern`). A context answers to its own name (current or original) with itself. The
  master found is kept in an `Rc` (masters are never parsed and never change, so no `Rc` cycle
  can form), and the reference parses `master.renamed(own name)`, its instance. A clone of a
  reference keeps the master (v12 `_cachedPattern`) but not the instance, so every recursion
  level parses a fresh one.
- **The implicit scope is built once per root.** `exec` needs it only when the root's tree
  holds a reference. It is v12's `findPattern(root, …)` as a table: depth first, children before
  parent, never into a reference, first valid match per name. Building it and checking every
  name costs a walk of the grammar and clones of its named patterns, so the result is cached on
  the root tree's first reference, keyed by the root's id and name. A root moved into a bigger
  pattern (different id) or cloned (a clone drops the cache) is readied again. Each later `exec`
  only walks to that first reference, without allocating.
- **Unknown names are failure as data, found before parsing.** `Pattern::check()` walks every
  reference a parse could reach, following each into what it would resolve to with the scopes it
  would have then, and returns `Err(PatternError::UnknownReference { name, target })`. `exec`
  runs it (once per root, as above) and, on an error, parses nothing and returns it in
  `ParseResult::pattern_error`. *Why not lazily, like v12, which throws only when the parse
  reaches the name?* §3 settles unknown names as build errors; a grammar that works on one input
  and breaks on the next is a trap; and v12 is not consistently lazy either (its choice resolves
  its references when it builds, and after one throw a second exec quietly stops throwing). So
  cpat-rs reports it up front, on every exec, even for empty text. Driving `parse` directly skips
  the check: an unresolvable reference then records an ordinary error at `(pos, pos)` and puts the
  reason on `Cursor::pattern_error()`. Never a panic.
- **The implicit scope is for `exec`/`test` only.** A direct `parse` sees context scopes, not the
  root's tree: a pattern has no parents to find its root from (§2). v12's `parse` on a nested
  pattern still searches up to the root. Wrap the pattern in a `Context` to parse it directly.
- **A context passes its name to its child when it becomes a child.** v12's composites clone
  their children, and `Context.clone()` renames its child to the context's name, so nodes from a
  context nested in anything are named after the context, while a context run directly keeps
  its child's names. Rust composites take children over without cloning, so they call
  `Pattern::adopted`, which does exactly that rename: the same names, decided at composition
  time. `Pattern::renamed` on a context renames its child too (v12 `clone(name)`), which is why a
  reference to a context names the nodes after the reference.
- **`exec` on a context** names its child in the summary error, since v12's `Context.exec` runs
  its child's exec.
- **The end-of-text lookahead probe** keeps the cursor's scopes and ancestors, as v12's probe
  keeps its parent chain, so `!ref` at the end resolves (`("a", "b"?, !o)` with `o` the optional:
  the lookahead fails, as in v12).
- **Checked against v12** by running both on 19 patterns (recursion through sequences, choices,
  repeats and contexts; left and mutual recursion; cuts through references; context lookup
  order; lookaheads at the end) and 62 inputs, each with a fresh pattern and again reusing one
  instance for every input: same trees, errors and furthest errors. `a = a "b" | "b"` is
  compared with the reference renamed in v12, because there a sequence starting with a child
  named like the choice becomes a postfix operator (the Pratt mode, a later milestone).
- **Deviations from v12:**
  - Unknown names are reported up front by `exec` (above).
  - `exec` on a pattern taken out of its tree (v12's 'Exec' test execs a reference found inside
    `value`): Rust patterns have no parents, so the reference names nothing and says so; in v12
    it resolves through its old parents.
  - v12's `Expression` hides its alternatives from `findPattern` until its first parse (its
    `children` are empty until it builds), so a reference parsed before a sibling choice can't see
    inside it. The implicit scope here always sees every alternative.

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
