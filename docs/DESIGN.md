# cpat-rs design

> **Status:** decided 2026-10-11. Items 1–9 are settled. Do not re-propose an alternative
> without new evidence. Milestone 1 (cursor, tree, Literal, Regex, Sequence, ordered Choice,
> Optional, Not, Cut, Custom, wasm layer) is implemented. Milestone 2 (`Repeat`, unbounded and
> bounded, with dividers, `trim_divider` and cut restoration) is implemented; see §10.
> Milestone 3 (`Reference`, `Context`, the ancestor stack, recursion guards, the implicit scope)
> is implemented; see §11, including its fix rounds (a depth limit, a check that can't blow
> up, parse-time halts, and implicit-scope targets resolving their names at home, including
> references wrapped inside a choice target, a documented deviation).
> Milestone 4 (the choice's Pratt mode: classification, the precedence tree, `RightAssociated`,
> the atom recursion guard in both modes, the end-of-text operator cut) is implemented; see §8
> for when a choice classifies and how `RightAssociated` is represented, and §12 for the rest.
> Its fix round made every walk over a finished tree iterative, so an operator chain as long
> as the input can't overflow the stack (§7e).
> Milestone 5 (the choice's `LiteralSet` mode: a first-character index that leaves the same
> winner, error and furthest error as the ordered loop) is implemented; see §8 for the key and
> §13 for the rest.
> Milestone 6 (`TakeUntil` and `Block`, with node kinds `TakeUntil`, `Block` and `BlockContent`
> in Rust and JS) is implemented; see §14. Its fix round keeps `\r\n` one grapheme in ASCII
> text (§5c) and pins the block's rewind after a failure past the open.
> Milestone 7 (cursor history: the recording parse, leaf matches, and a `PatternPath` on every
> recorded match and error, replayed with `Pattern::at_path`) is implemented; see §2 for the
> path and its cost, and §15 for the rest.
> Milestone 8 (introspection without parent pointers: tokens, patterns, what comes after a
> child and what comes next, by path; `find`, `is_equal`, `clone_named`, `Tokenizable`, and
> `generate_error_message`) is implemented; see §16. Its first fix round counts the query depth
> limit from the asked-about place, so a leaf deeper than 256 steps still gets its hint; its
> second refuses a path longer than any parse under the same `Limits` makes, so a long
> caller-supplied path can't overflow the stack.
> Milestone 9 (`AutoComplete` and `Suggestion`: v12's algorithm over a recording parse and the
> milestone-8 queries, and JS `localeCompare` order) is implemented; see §17. It also makes the
> leaf matches' tie rule treat a take-until's terminator as v12 does, parentless (§15). Its
> fix round pins the error-end filter and the furthest-start drop with tests (§17).
> Milestone 10 (the grammar core: the cpat grammar written by hand, `Grammar::parse_string` and
> the builder for every rule, without imports, params or decorators) is implemented; see §8 and
> §18. It also spells an ASCII `\b` for the backtracking regex engine (§6) and builds a context
> found by its own name only when a lookup needs the pattern (§18). Its fix round bounds how
> deeply a grammar's text may nest (10 levels of parentheses, 256 patterns built inside one
> another) so a deep text is a `GrammarError` rather than a stack overflow or an exponential
> build, shares each compiled regex between a pattern's clones, and builds every grammar from
> an unparsed copy of the cpat grammar (§18).
> Milestone 11 (imports through a synchronous `ImportResolver`, `with params` and `use params`,
> decorators with the built-in `@tokens`, `Grammar::import`, and the `patterns` convenience) is
> implemented; see §9 and §19. Every test in v12's `Grammar.test.ts` is ported (the checklist
> heads `tests/grammar.rs`). A grammar that imports itself is an error here, where v12
> overflows the stack (§19).
> Milestone 12 (the editable tree: `Tree::to_mut` into an `EditableTree` with v12's whole `Node`
> API by `NodeRef` handle, `ast::compact`/`ast::remove` by name, and the read-only tree's walks,
> breadth-first finds, `flatten` and `to_json`) is implemented; see §7b and §20. Every test in
> v12's `Node.test.ts` is ported (`tests/node.rs`). The read-only `find` now follows v12's
> quirk: a matching ancestor of the first match takes over (§20).

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
only while the start equals the current position. *(Implemented in milestone 3:
`Cursor::ancestors()`, see §11.)*

- **Paths instead of parent pointers** *(decided in milestone 7)*. A recorded match or error
  names its pattern instance by a `PatternPath`: the child index taken at each level from the
  exec root (`Rc<[u32]>`, so the one error kept as the current error, the furthest error and two
  history lists is one allocation). Each frame's `child` field is the index of the child that
  composite is on; every composite with more than one child stores it before parsing a child
  (`Cursor::set_child`). The path of whatever records next is therefore the frames' `child`
  values, read on demand, never built while nothing keeps one. `Pattern::at_path` replays a
  path on the root after the parse; the numbering per kind is listed there (a reference's child
  0 is its resolved instance, a Pratt choice numbers its classified alternatives in declaration
  order, an unbounded repeat is `[child, divider]`, a bounded one has v12's per-slot list, a
  block `[open, content?, close]`, `RightAssociated` is no step). The ancestors of an instance
  are exactly the prefixes of its path, which is what v12's leaf-match rule asks of `parent`.
  *Why indices:* a parse can't hold references into patterns it is mutating (§1), ids are
  shared by clones and so don't tell instances apart, and an index route is what v12's
  `parent`/`children` chain is, read top-down. *Rejected:* storing the path on each frame
  (a `Vec` per frame, cloned per push), and capturing a path at every record (an allocation per
  failed attempt).
- **The cost we're accepting** (measured, release build, against milestone 6): a store per child
  in every composite, a path copy (depth × 4 bytes into a reused buffer) whenever a match ties
  or beats the furthest end, and `ParseError` growing by an `Option<PatternPath>`. The path copy
  buys the furthest match its path in every parse, so an error message or autocomplete can
  start from it without recording. On a 60 000-token arithmetic
  expression (Pratt, references, groups) an ordinary `exec` is about 10% slower (97 → 107 ms
  for 20 runs); on 50 000 keywords through a literal-set choice in a repeat, where nearly every
  match is a new furthest one, about 30% (27 → 35 ms). Nothing allocates once the buffers are
  warm; with leaf tracking switched off the second case was 29 ms, so the copy is most of it.
  A recording parse adds an allocation per record, as v12's history does.

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
  uses one bit per byte. ASCII text containing `\r\n` takes the table too: UAX #29 (GB3) keeps
  CRLF as one grapheme, as v12's `Intl.Segmenter` does, and the all-boundaries shortcut would
  let `TakeUntil` stop between the two bytes, only when the rest of the text happened to be
  ASCII (found by the M6 verifier). CR followed by LF is the only multi-byte grapheme ASCII can
  form, so no other ASCII text needs the table. As in v12, literals `"\r"` then `"\n"` can't
  split a CRLF either (`tests/literal.rs`).
- **5d:** JS gets UTF-16 offsets, converted at the wasm boundary.

## 6. Regex

`enum Matcher { Fast(regex), Fancy(fancy_regex) }` is chosen when the pattern is built. The
linear-time `regex` crate is used unless the source needs lookaround or backreferences. The
pattern compiles as `^(?:src)` and matches on `&text[pos..]`, so `\b` and lookbehind treat the
cursor as the start of input, the same as the JS substring. `regex_translate` rewrites JS-only
syntax (ASCII `\d \w \b`, class escapes, `[^]`, `\0`, `\/`). An ASCII `\b` is spelled per engine:
`(?-u:\b)` for `regex`, lookaround over `[0-9A-Za-z_]` for `fancy-regex`, which refuses the
inline flag *(milestone 10, §18)*. A source that won't compile is a
build error naming the pattern.

- **Rejected:** calling JS `RegExp` from wasm. It would make native and browser results differ
  and cost a boundary crossing per call.
- **The cost we're accepting:** wasm size. The milestone-1 release build is 1.3 MB, mostly the
  two engines' Unicode tables. Measure before trimming features.

## 7. The tree

- **7a:** `Tree` is one flat arena: `Vec<NodeData>` plus a child-id pool. Names are interned
  (`Name` is a `u32`), and values are slices of the input.
- **7b:** an editable form (`to_mut`) gets built only when something needs to edit a tree.
  *(Implemented in milestone 12, §20: an arena of owned slots, addressed by `NodeRef`, whose
  leaves keep borrowing the input until an edit replaces their value.)*
- **7c:** the arena only grows during a parse. Nodes from failed alternatives stay in it,
  unreachable. History stores copies (`Match`, `ParseError`), not node references, each with
  its `PatternPath` (§2). *(Implemented in milestone 7, §15.)*
- **7d:** JS gets flat `Uint32Array`s with a thin `Node` view class (`js/tree.mjs`), so walking
  the tree never calls into wasm.
- **7e:** no walk over a finished tree recurses: writing an expression tree, `find`/`find_all`,
  the wasm flattener and the JS view's `find`/`findAll` keep their own stacks, and a node's
  `Debug` is shallow (its fields and child count). The depth limit (§11) bounds only the
  parser's recursion; a choice in Pratt mode loops, so an operator chain (`a+a+…+a`, `--…-a`)
  builds a tree as deep as the input is long. A recursive walk aborted the process on such
  input (fix round 1 of milestone 4).

## 8. The grammar *(milestones 10 and 11)*

- The cpat grammar is written by hand with the Rust API (`grammar/cpat.rs`, `cpat_grammar()`).
  As a test, cpat's own grammar is written as a `.cpat` file and built by the builder
  (`tests/fixtures/cpat.cpat`, §18). *(Implemented.)*
- The builder walks the read-only tree and flattens nested `+`/`|` on the fly. *(Implemented:
  `syntax::flattened`.)*
- Synchronous core with an `ImportResolver` trait; JS fetches imports ahead of time.
  *(Implemented in milestone 11, §19.)*
- Errors are values (`GrammarError`). *(Implemented, §18.)*
- Anonymous names are counted per grammar. *(Implemented.)*
- **One `Choice` pattern** with modes `Ordered`, `LiteralSet` and `Pratt`, picked from the
  classified alternatives (references resolved one level, alias cycles detected by the builder).
  There is no `Options`/`Expression` split and no greedy `<|>`. *(Milestone 10: the builder
  leaves a choice with reference alternatives to classify at its first parse, as v12 does; see
  §18 for why it can't decide earlier.)*
- **When a choice classifies** *(decided in milestone 4)*. In `Choice::new`, unless an
  alternative (seen through `RightAssociated`) is a `Reference`; then at the choice's first
  parse, resolving the reference one level against the cursor's scopes, as a `Reference` would
  (`Choice::classify_with` lets the grammar builder do it earlier with the definitions it
  holds). `Choice::mode()` is `None` until then. *Why:* only a sequence can be an operator, and
  whether a reference's target is one is known only once the name resolves; the scopes that
  decide that exist only while parsing, and a Rust pattern has no parents to search from (§2).
  Everything else is decided where the pattern is built, so a grammar mistake that needs no
  scope (an alternative that is nothing but a self-reference) is a build error, and a choice
  with no references never branches on "classified yet?" again. A clone keeps the
  classification (so a renamed reference instance doesn't redo it), and classification reads
  the name the choice was built with (v12's `_originalName`), so a renamed clone that classifies
  late still recognises its self-references. *Rejected:* v12's "classify every clone from the
  original alternatives": its references cache their resolution on the shared originals anyway,
  so the result is the same, and re-deriving it per clone only costs. *The cost we're
  accepting:* a master with reference alternatives (masters never parse) is classified again by
  each recursion level's instance on its first parse, as v12 builds each clone.
- **`RightAssociated` is a pattern, not a flag** *(decided in milestone 4)*: a
  `Pattern::RightAssociated` variant wrapping one child, as in v12. It has no name (`""`),
  pushes no ancestor frame (v12 doesn't make it its child's parent), ignores renames (v12's
  `clone(name)` does), and parses, tests and execs as its child. Only classification reads it:
  an infix alternative wrapped in it associates right, and its precedence is keyed by the
  child's name. *Why a variant:* the marker travels with the alternative it marks, through a
  vector of alternatives, a reference (`right(reference("pow"))`) and, later, the grammar
  builder, and every exhaustive `match` on `Pattern` has to say what it does with it.
  *Rejected:* a flag per alternative on `Choice::new` (a second list kept in step with the
  first by position, and impossible to put on a referenced definition), and a separate
  constructor for operator choices (a second way to build the one choice).
- **The literal-set index is keyed by the first `char`** *(decided in milestone 5)*. A choice
  with no operators whose atoms are all literals (directly, or through one reference resolved
  to a literal) takes `ChoiceMode::LiteralSet`. *Why `char`, and why it gives the same winner:*
  `Literal` compares `char` by `char`, so a literal whose first `char` differs from the text's
  fails on its first comparison, at the start index, the least `last` a failure can have. Only
  the bucket sharing the text's first `char` can win or fail further, and its literals are
  compared with `Literal`'s own `mismatch`, so the rule can't drift. The first bucket literal
  that matches is the first literal in declaration order that matches, since every literal
  outside the bucket fails. A first-byte key is equally correct (a differing first byte means a
  differing first `char`) but would put literals that share only a UTF-8 lead byte (`é`, `è`) in
  one bucket, to fail at the start anyway. The index is a sorted `Vec` searched by binary
  search, built once at classification.
- Shadowed literals, unknown names and alias cycles are build errors. *(Implemented, §18.)*

## 9. Decorators *(milestone 11; JS decorators with the wasm grammar milestone)*

- A decorator is `Fn(Pattern, Option<&Json>) -> Result<Pattern, String>`. It can wrap or replace
  a pattern, for example to inject a `Custom` one. *(Implemented: `Decorator`, `Decorators`,
  `cpat_core::Json` is `serde_json::Value`; §19.)*
- Mistakes are build errors. *(Implemented: every decorator error is `[Invalid Grammar] …`.)*
- JS decorators are fully supported. A JS custom pattern uses a minimal contract,
  `(text, pos) => end | null`, so each attempt is one boundary call. *(With the wasm grammar
  milestone.)*

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
  the dispatch `match`, so no composite can forget to. Leaves push nothing. `child` was 0 until
  milestone 7, which fills it so the stack spells the path of whatever records next (§2). Guards ask "how many frames with this id
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
- **An implicit-scope target resolves its own names at home** (lexical binding). v12's
  `findPattern(root)` returns the LIVE pattern in the root's tree, and cloning it keeps what its
  references already resolved to there. So once the definition has parsed, a reference from
  anywhere sees the names of the contexts around the definition: `t` inside a context `k` that
  defines `u`, reached by `rt` from outside `k`, matches `k`'s `u`, even when a context around
  `rt` defines `u` differently. The first version cloned a pristine `t` and resolved its `u`
  where `rt` sits, so that grammar was `UnknownReference` on every input (or matched the outer
  `u`). Now each implicit entry records its *home*, the contexts around it in the root's tree,
  and a target found there parses under `[implicit scope, home contexts…]` instead of the
  reference's stack (`scope::Found::home`, `Cursor::within_scopes`, which raises a floor so
  lookups can't see the frames below). The check walks it under the same frames. A context's
  own definitions have no home (they sit in no tree, and v12 resolves them where they are used),
  nor does a context found by its own name (v12's `Context.clone` clones its child twice, and
  the second copy drops what its references resolved). A home applies only under the implicit
  scope it was found in: a resolved reference cloned out of its root, or a renamed root, falls
  back to resolving where it now sits (as v12 resolves whatever its cached pattern had not), and
  `Pattern::check` reuses the scope `exec` readied so the homes it recorded still apply.
  *Why not v12's exact rule?* There isn't one: v12's answer depends on parse history (whether
  the definition had parsed before the reference, in this or any earlier `exec`, since the
  resolution persists on the instance), and on the target's shape: v12's `Expression.clone`
  rebuilds a choice from its original, never-parsed alternatives, so a reference wrapped in a
  sequence, optional or repeat beneath a choice resolves where the target is used even when the
  definition parsed first (a reference that is itself an alternative keeps its home). Home
  binding is the one lexical rule behind what v12 does when the definition parses first and the
  choice clone doesn't drop it, and it gives one answer for every input and every shape. Matching
  v12 would mean reproducing an artifact of how its choice clones, where wrapping `u` in a
  sequence changes which `u` it means. Checked against v12 on sequence, choice (direct and
  wrapped alternatives, with and without a cut), optional, repeat, recursive and top-level
  targets: same results except the two deviations listed below.
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
  cpat-rs reports it up front, on every exec, even for empty text.
- **The check can't blow up.** It is iterative between references (a reference queues its
  target instead of recursing), so a chain of thousands of references costs heap, not stack. It
  skips a target already walked under the same *resolution*: what every name in scope resolves
  to (`scope::resolution`), not the order of the frames. The first version keyed on the ordered
  frame stack, which grows with the orderings of nested contexts: 7 contexts that all refer to
  one another overflowed the stack, which v12 handles at 12 (`tests/reference.rs`
  `check_handles_many_mutually_referring_contexts`). Distinct resolutions can still grow
  exponentially in pathological grammars, so the check walks at most 10 000 states
  (`CHECK_BUDGET`) and then passes, leaving the rest to the parse. Resolution ids are cached
  per frame stack, so a reference costs one hash of a pair.
- **A parse that meets an unknown name halts** (`Halt::Pattern`): v12's throw, as data. This is
  reached when `parse` is driven directly (no check runs) or past the check's budget. The
  reference records its error at `(pos, pos)`, `Cursor::pattern_error()` gives the reason, and
  `exec` returns it with no tree. Halting rather than failing matters: as a plain failure, a
  later alternative could match and hide the broken grammar (`choice(ref missing, "t")` on
  `"t"`).
- **Depth is bounded: input can't overflow the stack.** Through references the parse depth
  follows the input, so `expr = "(" expr ")" | "x"` on 10 000 levels aborted the process
  natively (Rust can't catch a stack overflow) and, in wasm, left the module broken for every
  later call ("memory access out of bounds"). Every composite frame counts against
  `Limits::max_depth`, checked where the frame is pushed (`Cursor::framed`); past it the parse
  halts (`Halt::TooDeep`) and `exec` returns `ParseResult::too_deep` with no tree (`tooDeep`
  in JS). The default, `DEFAULT_MAX_DEPTH = 1024`, was measured: about 540–600 bytes of stack per
  frame in a debug build and under 400 in release, so 1024 frames fit a 2 MB test thread and
  the 1 MB wasm stack with room for custom code, while still allowing about 340 levels of the
  grammar above or 200 of a JSON-style array grammar (serde_json stops at 128).
  `Pattern::exec_with(text, Limits { max_depth })` and `Cursor::with_limits` raise it, for
  callers whose thread has the stack. *Rejected:* growing the stack on demand (`stacker`) does
  not work on wasm32, and a byte budget measured from stack addresses would make the same input
  pass natively and fail in wasm. A frame count gives the same answer everywhere.
- **How a halt unwinds.** Once halted, `Pattern::parse` fails every pattern on entry, and
  `record_error`/`resolve_error` stop changing the error, so every parent fails in turn without
  trying alternatives (a fallback alternative must not turn "too deep" into a match). A
  lookahead's probe that halts carries the halt to the real cursor (`Cursor::halt_from`).
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
  named like the choice becomes a postfix operator (the Pratt mode, a later milestone). The fix
  round added v12-checked tests for four behaviors that no test pinned before (each found by a
  mutant that survived): the implicit scope searches children before their parent, a context
  answers to its original and its current name, a clone of a resolved reference keeps its
  master, and a lookahead pushes the frame the choice guard reads.
- **Deviations from v12:**
  - Unknown names are reported up front by `exec` (above).
  - An implicit-scope target always resolves its names at home (above). When v12 parses the
    reference before the definition has ever parsed, it resolves them from the reference's place
    instead: `rt` before `k`'s `t` matches the outer context's `u` in v12 (or throws
    "Couldn't find 'u'" when only `k` defines it), and `k`'s `u` here. Pinned in
    `tests/reference.rs` (`deviation_a_reference_parsed_before_the_definition_…`).
  - A reference wrapped (in a sequence, optional or repeat) beneath a choice inside an
    implicit-scope target also resolves at home. v12 rebuilds the choice from its never-parsed
    alternatives on every clone, so it resolves such a reference from the place the target is
    used even after the definition has parsed: `t = ("a" u) | "ab"` in `k`, reached by `rt`
    from outside, throws "Couldn't find 'u'" in v12 on every input, or matches an outer
    context's `u` when one surrounds `rt`; here it means `k`'s `u`. (Found in fix round 3; the
    second fix round's home binding introduced the difference, the version before it threw like
    v12.) Pinned in `tests/reference.rs`
    (`deviation_a_reference_wrapped_inside_a_choice_target_…`).
  - `exec` on a pattern taken out of its tree (v12's 'Exec' test execs a reference found inside
    `value`): Rust patterns have no parents, so the reference names nothing and says so; in v12
    it resolves through its old parents.
  - Input nested past `Limits::max_depth` halts with `too_deep`; v12 recurses until the JS
    engine throws `RangeError` (about 3000 levels of `expr`). The limit is lower than v12's
    reach on purpose (above).
  - A parse that reaches an unknown name halts with `pattern_error` instead of throwing.
  - A `Sequence` that runs out of text records its error's `last` at the end of the text. v12
    records `cursor.index + 1`, one UTF-16 unit past the start of the last grapheme, which is
    inside the grapheme when it is wider than one unit (`("é", "x")` on decomposed `"é"`: v12
    `root@0..1`, here the end of the text; the same for a lookahead blocked at the end). Positions
    here are always grapheme boundaries (§5c), and the `+ 1` is v12 assuming one-unit
    characters. On ASCII the two agree. Pinned in `tests/sequence.rs`. (Found in this
    milestone's fix round, but it dates from milestone 1.)
  - v12's `Expression` hides its alternatives from `findPattern` until its first parse (its
    `children` are empty until it builds), so a reference parsed before a sibling choice can't see
    inside it. The implicit scope here always sees every alternative.

## 12. The choice's Pratt mode *(milestone 4)*

- **Classification** ports `classifyExpressionPatterns` exactly. Seen through `RightAssociated`
  and a reference resolved one level, an alternative that is a sequence whose last child (only)
  is named like the choice is a prefix operator (`<name>-prefix`, the sequence without that
  child); first child only, a postfix (`<name>-postfix`); both, with more than two children, an
  infix (`<name>-delimiter`, left-associative unless marked). A self-reference is a child with
  the choice's name, whatever kind of pattern it is. Precedence is the declaration index, keyed
  by the alternative's name (a later alternative with the same name replaces the entry). A
  sequence of only the self-reference is `PatternError::SelfOnlyAlternative` (v12 throws
  "Need at least one pattern" building its prefix): from `Choice::new`, or as a parse halt
  (`ParseResult::pattern_error`) when found through a reference. Any other sequence that starts
  and ends with the choice (`e e`) is neither operand nor operator and is never tried, as in
  v12. No operators: `ChoiceMode::Ordered`, the M1 loop over the atoms.
- **Operands are the alternatives themselves, parsed in place**, by index. v12 parses clones;
  a clone shares the id and starts with fresh state, so nothing observable changes and nothing
  is cloned. Operators are new `Sequence`s with fresh ids; each parses in its own ancestor frame
  (v12 makes the choice its parent), so they're on the chain the recursion guards read.
- **The parse loop** (`pratt.rs`) is v12's `_tryToParse` phase for phase: prefixes (restarting
  after each match), one operand, postfixes (restarting), then an infix, and round again.
  Every successful attempt clears the choice's cut flag (`Cursor::clear_cut`); every failed one
  checks it and, when set, stops with a committed failure that keeps the deep error and drops
  whatever was assembled. An operand or postfix that ends the text tries the postfix and infix
  operators with `Sequence::child_blocked_at_end_of_text`, the walk `finish_at_end` makes from
  child 0, recording nothing; when that fires a cut, the committed operator records its error on
  the blocked child at the end of the text. The atom recursion guard (§11) applies in both
  modes. A halt inside the loop fails the choice even when an operand was already assembled.
- **The precedence tree** (`precedence.rs`) assembles in a scratch tree of slots with v12's
  node operations (`append`, `remove`, `replace_with`, root-finding) and placeholders, step for
  step, including the right-associated equal-precedence stop when climbing, the revert of a
  trailing infix with no right operand (data, not closures), and the prefix/postfix order (the
  lower declaration index goes inside; ties keep the prefix inside). Only the result is written
  to the arena, as `NodeKind::Expression` nodes named after the operator alternatives, children
  first. Prefix and infix operators contribute their sequence node's *children*; the sequence
  node stays in the arena, unreachable, and its children are re-parented to the expression node
  (the only re-parenting the arena does). An operand with no operators comes back unwrapped. An
  expression node spans its first child's start to its last child's end; v12's `normalize`
  recomputes ranges from the concatenated values, which is the same span whenever the children
  are contiguous (they always are, short of the nodeless-child gaps §10 discusses).
- **Known limitation, kept:** a prefix that consumes the last token doesn't backtrack, so
  `x = "a" x | "a"` rejects `a` (pinned in `tests/expression.rs`).
- **Fixed relative to milestone 1:** a committed alternative that left no error (it matched
  nothing: a cut and absent optionals) leaves the choice matching nothing, with no error, in
  both modes, as in v12. Milestone 1 recorded the choice's own error there, an untested guard
  with no recorded reason; inside a sequence that turned v12's match into a failure.
- **Checked against v12** by running both (one shared spec, a TS runner and a generated Rust
  one) on 45 grammars and 180 inputs with one instance reused per grammar: the Expression,
  RightAssociated and PrecedenceTree test grammars, Pratt-mode cuts (leading, trailing, in
  prefixes, postfixes and operands), reference alternatives (plain, renamed, marked, choices),
  dropped alternatives, Pratt choices inside sequences, repeats and lookaheads: same trees,
  errors and furthest errors. The milestone-3 comparison suites still match except for its
  documented deviations; its left-recursion cases now parse as postfix chains, as in v12. A
  mutation pass removed each mechanism above in turn (guards, every `clear_cut` and cut check,
  the end-of-text probe, association, the climb stop, the revert, the order swap, reference
  resolution, the original name, frames, the halt check); each made a test fail.
- **Deviations from v12:**
  - The end-of-text operator error sits at the end of the text; v12 records it at `index + 1`,
    inside a last grapheme wider than one UTF-16 unit (`😀`). Same reason as the `Sequence`
    case in §11; pinned in `tests/cut.rs`.
  - A renamed clone of a choice whose alternatives refer to its old name names nothing it can
    see, and `exec` says so up front (§11). v12 throws only when a parse reaches such a
    reference; its 'Clone With New Name' test passes because its input never does. Pinned in
    `tests/expression.rs` (`clone_with_new_name`).
  - An operator chain of any length parses (§7e). v12 builds and walks its tree recursively
    and throws a `RangeError` once the chain outgrows the JS stack (a few thousand operators
    when `toString` walks the result). Same trees where v12 finishes: compared on 500-operator
    chains of each kind and on mixed chains. Pinned in `tests/limits.rs` and
    `js/tree.test.mjs` with 100 000 operators.
  - Operator parts resolve their references at home like any implicit-scope target (§11). v12
    builds them from never-parsed alternatives, so where it throws "Couldn't find" for a
    reference inside an operator of a target reached from elsewhere, the name resolves here.
    This is the §11 deviation for references beneath a choice, met through an operator.
- **Deferred:** introspection (`getTokens`, `getPatterns`, `…After`, `find`) to the
  intellisense milestone, which will also skip an atom already being expanded higher up (done
  in milestone 8, §16); the
  `LiteralSet` mode and its tests to M5 (done, §13); the grammar-file cut tests (`~/` in `.cpat`, the
  shell-shaped blow-up test) to the grammar milestone (done, `tests/grammar_cut.rs`).

## 13. The choice's LiteralSet mode *(milestone 5)*

- **What it reproduces** (v12 `LiteralSet.match` and `Expression._parseLiteralChoice`): the
  ordered loop's winner (the first literal in declaration order that matches) and the failure
  the loop would leave as the furthest error, the FIRST failed literal with the greatest `last`
  (the cursor replaces the furthest error only on a strictly greater `last`). Every literal
  outside the bucket fails at the start; when no bucket literal got further, the first literal
  tried is that failure. Only that one error is recorded, then only the winner is parsed; with
  no winner the choice records its own error at `(first, first)`, as the loop does. Nothing
  else a failed literal does survives the choice (it resets the cursor either way, which also
  forgets the spot a failed `Literal` notes with `stopped_at`, §10), so the results, error,
  furthest error, leaf match and cursor are the loop's.
- **The winner is parsed in place, as the loop would parse it.** For a reference alternative
  that means through the reference: it pushes its frame and names the node after itself. v12
  parses the reference's resolved clone directly, which is named after the reference too, so
  the result is the same. A failed literal reached through a reference is recorded under the
  reference's instance (the target's id, the reference's name), as the loop would record it.
- **Not recorded, and not resolved.** v12 records the furthest failure and resolves it at once.
  The resolve is left out: on success the winner and the choice resolve anyway, and on failure
  the choice records its own error over it, so it could never be seen.
- **The plain loop still runs at the end of the text** (v12 does too): every literal fails there
  without comparing anything. The check lives in one function, `literal_set_applies`, which is
  also where milestone 7's recording parse will send a choice back to the loop (a recording
  parse keeps every literal's error, which only the loop produces).
- **`Choice::ordered` is the plain loop, chosen by the constructor.** It classifies like
  `Choice::new` but never takes the literal-set mode, so tests can compare the two on the same
  alternatives (v12 compares with a recording parse, which doesn't exist yet). The choice keeps
  which constructor built it (a private two-variant enum read only when classifying, which for
  a reference alternative is the first parse), so parsing never branches on it. *Rejected:* a
  public "mode" argument: Pratt can't be forced, and `LiteralSet` can't be forced either (it
  needs literals), so the only real choice is "allow the index or not".
- **No cut scope.** A literal can't cut, so the literal-set path skips `in_choice_scope`.
- **No atom recursion guard**, as in v12's fast path. A literal never pushes a frame. A
  reference alternative does, but its instance is a literal, so a frame with its id can begin
  at this position only from another instance of the same reference resolving the same name
  to something that contains this choice, through a context between them redefining it. The
  loop would skip that alternative; the index doesn't, exactly as in v12.
- **Observing the parses.** v12's tests count `Literal.prototype.parse` calls with a jest spy.
  The `parse-log` feature (off by default, on for this crate's own tests through a dev-dependency
  on itself) logs every `Literal::parse` on a thread-local list, `parse_log::take_literals()`;
  the tests assert that sequence and its length. *Why a feature:* always on, it would cost
  every literal a push.
- **Checked against v12** by running both (a TS script against v12's `src` and a Rust test) on
  5 grammars (the keyword choice alone, inside a sequence, after a literal, a multi-byte and
  emoji choice, and literals reached through references in a context) and 44 inputs, each in
  v12's fast and plain modes and in both modes here: same trees, errors and furthest errors,
  except the deviation below. `tests/literal_set.rs` pins v12's values for the ported tests,
  and its differential test compares `LiteralSet` with `Choice::ordered` on 6 choices
  (duplicates, shared prefixes, longest-first, multi-byte, emoji, a decomposed accent) over
  every prefix and one-character mutation of every token (2 915 inputs), parsed from the
  start, after a literal, at the end of the text, under `exec`, and inside a repeat. A mutation
  pass removed or bent each rule above (strict `>`, the start-index fallback and its `<=`, the
  first-winner `break`, the recorded error, the reference's name, the reference arm, the
  non-literal atom reset, the marker); each made a test fail.
- **Deviations from v12:**
  - A literal whose first character shares only a UTF-16 high surrogate with the text's (`😀a`
    against `😁`) gets one unit in, inside the emoji, in v12 (its `Literal` compares UTF-16
    units), and that failure beats every literal that failed at the start. Here `Literal`
    compares `char`s, a position inside a `char` doesn't exist (§5), and it fails at the start
    like the rest, so the furthest error is the first literal's. This is milestone 1's
    `Literal`, met through a choice; both modes agree, as they do in v12. Pinned in
    `tests/literal_set.rs` (`deviation_a_literal_sharing_only_a_high_surrogate_…`).
  - At `Limits::max_depth`, an ordered loop that tries a reference alternative before a
    literal winner halts on that reference's frame; the index never parses the reference, so
    the literal wins. The limit guards the stack (§11), and the index uses none. v12 has no
    limit. Pinned in `tests/literal_set.rs` (`deviation_at_the_depth_limit_…`).

## 14. TakeUntil and Block *(milestone 6)*

- **`TakeUntil` is v12's loop on half-open positions.** At each grapheme it probes the
  terminator inside its own frame (v12 makes it the terminator's parent), then puts back the cut
  it saw before the probe, resolves the error and rewinds: a probe is never kept, so nothing it
  commits survives (Cut.test.ts's TakeUntil case is pinned in `tests/cut.rs`). It stops at the
  first grapheme where the terminator returns a node (a terminator that succeeds without one, an
  absent optional or a passing `!x`, never stops it, as in v12), or at the end of the text; it
  never probes at the end, as v12's `hasNext` never does. Taking nothing is an error at
  `(first, first)`. The node is a `TakeUntil` leaf whose value is the slice it spans, so it
  always covers whole graphemes. A probe that halts (an unknown name, too deep) ends the scan
  with a failure: scanning on would fail every later probe on entry, and the half-taken text
  must not come back as a match. `set_tokens`/`tokens` hold intellisense suggestions; a clone
  keeps them.
- **`Block` takes `Literal` delimiters, so the types enforce what v12's signature asks for.**
  They are kept as `Pattern`s inside (with their tokens copied out for the scan) so a grammar
  search sees `open`, content and `close` as v12's `children`; `Children::Few` grew to three
  slots for it. JS hands over any pattern, so `Pattern.block` checks there and throws naming the
  delimiter. Phases as v12: parse `open`; scan for the balancing close with `str::find` (opens
  before each close raise the depth; when the closes run out, the last close seen is taken);
  parse the content from the open's end; parse `close` where the scan found it. Adjacent
  delimiters are always valid (the base case of a recursive block). Content that returns no node
  fails the block unless it is an `Optional` (decided once at construction, as v12 checks the
  pattern's type), and content must end exactly at the close; it parses on the whole text, so a
  pattern that runs past the close fails. Every failure rewinds and records the block's error at
  `(first, first)` (v12 records at the cursor after rewinding, which is the same spot). The
  block doesn't restore the cut after failed content, as in v12: a cut there commits the choice
  around the block. A wildcard block (no content pattern) with text between the delimiters gets
  a `BlockContent` leaf named `<name>-content`; the name is interned once and kept in step by
  `Pattern::rename`. v12's `cursor.hasError` check after the content is left out: a failed
  pattern always returns no node, and the only content allowed none is an `Optional`, which never
  leaves an error, so the node alone decides (a mutant removing the check survived).
- **Checked against v12** by running both (a TS script against v12's `src` and a scratch Rust
  binary) on 30 patterns and 78 inputs each, through `exec` with a fresh and a shared instance
  and through `parse` from 0: take-until with literal, choice, sequence-with-cut, regex, `!x` and
  optional terminators, inside sequences and repeats, over combining marks and emoji; blocks with
  no content, regex, sequence, optional, `!x`, overrunning and cut content, nested, recursive
  through a reference, with multi-character and emoji delimiters, in sequences, repeats and
  choices. Same trees, errors and furthest errors except the deviations below and the known
  ones: a `Sequence` that runs out of text inside a wide last grapheme (§11), and a direct
  `parse` of a pattern whose references only `exec`'s implicit scope can resolve (§11). A
  mutation pass removed each mechanism (the cut restore, the halt check, the leaf match, the
  grapheme step, the optional allowance, the adjacent base case, the exact-end check, the
  wildcard node and its condition, the content name on rename, the content start, the block's
  error, the scan's fallback and depth, both frames, and both patterns' children for searches);
  each made a test fail. The fix round added the block's rewind to `first` (a direct `parse`
  that fails after the open must leave `pos` at the start, as v12's `cursor.index`) and CRLF as
  one grapheme in ASCII text (§5c); both now fail a test when removed.
- **Deviations from v12:**
  - A block whose content is an absent `Optional` keeps only `open` and `close`, skipping the
    text between them, as in v12. Its value here is the slice it spans (§10), so `exec` matches
    `{1}` when the content is an optional `[a-z]+`, where v12 compares the joined children
    (`{}`) with the text and fails. Pinned in `tests/block.rs`.
  - An open token that ends inside a grapheme (`e` against `e` + combining accent): the content
    starts at the next boundary, where the open literal left the cursor. v12's `moveTo` goes back
    to the grapheme's start, so its content overlaps the open and its `exec` fails. Positions
    inside a grapheme don't exist here (§5c). Pinned in `tests/block.rs`.
- **Deferred to M8 (intellisense), done there (§16):** `getTokens`/`getTokensAfter`/`getPatterns`/`…After`/`find`
  for both, and `isEqual` (Block.test.ts 'autocomplete introspection' and 'equality',
  TakeUntil.test.ts's introspection and equality tests). `Block::record_match` is called as in
  v12 but can't be observed yet: its close always ends where it does, and the leaf match keeps
  the first match with the greatest end. Milestone 7's recording parse will see it.

## 15. Cursor history *(milestone 7)*

- **Recording is chosen once per parse.** `Pattern::exec_recording` (v12 `exec(text, true)`)
  starts recording on the cursor before parsing; `test_recording` is its `test`. The cursor
  holds `Option<Box<History>>`, created only then, so an ordinary parse pays one never-taken
  branch per record and allocates nothing for history. While recording, the cursor keeps every
  error (`errors()`), every match (`matches()`, v12's `allMatchedNodes`/`allMatchedPatterns` as
  one list of copies), both in order (`records()`, `HistoryRecord::Match | Error`), and the root
  match (`root_match()`, the last match recorded). `stop_recording` stops appending and keeps
  what is there. As in v12, empty text is never recorded (`is_recording()` is false: v12
  returns before it starts recording), and exec's summary error is recorded last, naming the
  context's child with path `[0]` through each context (v12's `Context.exec` runs its child).
- **Every record carries its path; an ordinary error doesn't.** A recorded `ParseError` has
  `path: Some(…)`; outside a recording `path` is `None`, because almost every failed attempt
  records an error and a path per error would cost an allocation that only readers of the
  recording want. A `Match` always has its path: matches are kept only in the history and in
  the leaf matches. Which pattern a record names is said where it is recorded
  (`ancestors::At`): a leaf (no frame) is the top frame's current child, a composite owns the
  top frame, and a few patterns record for a descendant (a repeat for its child, a sequence for
  its blocked lookahead, a Pratt choice for an operator's blocked child, the literal set for an
  atom). The public `Cursor::record_error`/`record_match` are the leaf case, which is what a
  `Custom` pattern is.
- **Leaf matches** (v12 `leafMatches`) are kept in every parse, as in v12: the furthest match
  last, and in front of it, matches that ended at the same spot whose parent is no ancestor of a
  match already kept. With paths, "ancestor of m" is "at a proper prefix of m's path", plus the
  repeat facade below. Paths live in buffers recycled when a further match replaces the list,
  so the ordinary parse copies but doesn't allocate once warm.
- **v12's `Repeat` facade.** v12 wraps every repeat in a `Repeat` whose only child is the
  `InfiniteRepeat`/`FiniteRepeat` that parses and records, so that repeat's parent is a pattern
  at the same place: an ancestor of everything inside the repeat, and of the repeat itself, and
  nothing else. A repeat's match is recorded with `Parent::Facade`, which reproduces it. It is
  observable: `a+` on `"aa"` at the root keeps `[a]` in v12 (a sequence at the root would be
  kept beside its last leaf, since the root has no parent).
- **The literal set falls back to the ordered loop while recording** (`literal_set_applies`),
  closing the milestone-5 hook: v12 does the same, since only the loop records every literal's
  error. `tests/history.rs` asserts the recording parse's `errors()`, `records()` and leaf
  matches are the same as `Choice::ordered`'s.
- **The Pratt choice's end-of-text operator walk** now runs in the operator's own frame, as
  v12 walks it with the operator as the probes' parent, so a committed operator's error and a
  halting probe are recorded under the operator. The frame counts toward `Limits::max_depth`
  like any other.
- **API changes:** `ParseError` and `Match` are `Clone`, not `Copy`, since they carry a path
  (`Cursor::error()` and friends return clones). `Sequence::child_blocked_at_end_of_text` is
  now crate-private and returns the child's index.
- **Checked against v12** by running both (a TS script against v12's `src` printing each
  record's pattern, span and path, walking `parent` and `children.indexOf` and stepping over the
  facade, and a scratch Rust binary that also replays every path with `at_path`) on 28 grammars
  and 103 inputs, each plain and recording: the name grammar of `Cursor.test.ts`, literal sets
  (plain, in a sequence, through references), Pratt choices (every operator kind, right
  association, a leading cut at the end of the text), both repeats (dividers, trimming, at the
  root), lookaheads and optionals, sibling and ancestor ties, take-until, blocks (with and
  without content, nested), recursion through references and contexts, cuts. Same errors, same
  records in the same order with the same paths, same leaf matches, except the deviations
  below. A mutation pass removed each mechanism (every `set_child`, each `At` choice, the
  facade rule and its self case, the proper-prefix rule, the tie order, the literal-set
  fallback, the summary path, the history pushes, the marker skip in `at_path`); each made a
  test fail.
- **Deviations from v12:**
  - v12's `TakeUntil` never makes itself its terminator's parent (it doesn't call
    `_assignChildrenToParent`), so the terminator has no parent there and records with a path
    that stops at it. Here the terminator is the take-until's child 0 like every other child,
    so paths go through the take-until. The same missing link would stop a reference inside a
    terminator from searching past it in v12. *Changed in milestone 9:* the leaf matches' tie
    rule now follows v12's chain, which stops at the terminator (each kept match carries the
    length of the terminator's path it was recorded in, its `floor`, set by
    `Cursor::detached` around the probe). Milestone 7 made the terminator's parent the
    take-until there too, so on `"<" ?->| ">" ">"` with `"<ab>"` it kept `[tag, close]` where
    v12 keeps `[tag, close, close]`: the sequence's own `close` was dropped, and with it
    autocomplete's suggestion of what follows the closing delimiter (v12 offers `"."` after
    `"<abc>"` in `"<" ?->| ">" ">" "."`; the first version of milestone 9 offered nothing).
    The terminator's probe is lookahead that v12 records anyway, so the leaf list it leaves is
    v12's to match. Pinned in `tests/history.rs` and `tests/autocomplete.rs`.
  - `RightAssociated` deliberately doesn't parent its child in v12, so an atom wrapped in it
    has no parent there; here a path goes through the marker to the choice. Only the leaf-match
    tie rule could tell, and only for a marked atom (a marked operator is unwrapped into its
    operator sequence, which has the choice as parent in both).
  - A bounded repeat ends its walk at an idle lap (§10), so a recording parse records the
    failures of that lap once where v12 records them in every remaining slot (`(x?){1,3} "b"`
    on `"b"`: one `x` error here, three in v12). Results, errors and furthest errors are the
    same. Pinned in `tests/history.rs`.
  - v12 sets `rootMatch` in every parse; here it is part of the recording only (the last
    recorded match), since keeping it otherwise would copy a path per match.
  - A bounded repeat with `trim_divider` whose first child fails throws a `TypeError` in v12
    (§10), recording or not.
- **Not ported here:** `generateErrorMessage` (milestone 8, §16) and autocomplete (milestone 9, §17), which read the leaf match's
  pattern's tokens (`getNextTokens`), come with intellisense (milestone 8); the leaf match's
  path is what they will start from. v12's cursor-movement tests (`Move Cursor`, `Text
  Information`, `Empty Text`, `Text with Emojis`) are mapped onto `set_pos` and the snapping
  calls, since the half-open cursor has no `next`/`previous`.

## 16. Introspection *(milestone 8)*

- **Every question is asked about a place.** v12's introspection is defined by `parent`:
  "what comes next" asks the parent what follows this child, and "am I already being
  expanded" counts ancestors with my id. Rust patterns have no parents (§2), so a query names
  its pattern by a `PatternPath` from a root, and the answer names places too: a `Handle` is
  the path, the `&Pattern` there, and the name and id the instance answers to (a reference's
  target is renamed after the reference, as v12's resolved clone is). AutoComplete can ask
  about a handle in turn. `Pattern::introspect()` builds an `Introspection` over a root; the
  shortcuts on `Pattern` (`tokens`, `patterns`, `tokens_after(child)`, `patterns_after`,
  `next_tokens(path)`, `next_patterns(path)`, `find`) ask it about the root or a path.
  `tokens_after` takes a child index where v12 takes the child pattern; an index that names
  no child is v12's "bogus child" and gets nothing.
- **The parent chain is rebuilt for one query** (`introspect/loc.rs`). A `Loc` is a link:
  the place, its parent link, its index there, the scopes a parse would have there, and the
  id and name the instance answers to. Walking a path makes the chain top-down from the root;
  a query extends it as it expands patterns, through references too. The guards are v12's,
  read off the chain: a sequence's first child expanded twice above it offers nothing
  (`isRecursivePattern`), and a choice skips such an atom (the lesson that ends
  `a = b | "c"`, `b = a | "d"`: v12 gives `["c", "d", "c", "d", "c"]`, and so does this).
  Nothing is written into patterns, so a query takes `&Pattern`. *Rejected:* parent indices
  stored in patterns (they would have to be kept right through every clone and move, for a
  question asked rarely), and v12's lazy resolution that writes the resolved instance into
  the reference (it would make a query need `&mut`, and the instance it makes is never
  parsed).
- **Children are numbered as `Pattern::at_path` numbers them,** so a leaf match's path from a
  parse is a place a query can start from. A reference's child 0 is what it resolves to; a
  choice's are its classified children (operators as their parts, self-references left out);
  a bounded repeat's are v12's slots; `RightAssociated` is no step.
- **References resolve as the parse resolves them** (`introspect/frames.rs`), borrowed instead
  of cloned so a handle can point into the root: the instance or master a parse already found
  (v12 `_pattern`, `_cachedPattern`), with its home when that home belongs to the implicit
  scope in force; otherwise a lookup through the contexts on the chain, then the implicit
  scope, with the same home binding as the parse (§11). The implicit scope is the one an
  `exec` readied for this root when there is one (so homes found by the parse still apply,
  compared by identity); otherwise the root's live tree, searched in v12's `findPattern`
  order (`scope::visit_targets`, which `exec`'s scope is now built from too). `exec`'s scope
  can't be used for a root never exec'd: it holds clones, which a handle couldn't borrow.
- **A choice classifies with the parse's rule.** `classify::role_of` and `operator_part`,
  split out of classification, decide each alternative's role (resolving a reference one
  level), so a choice that hasn't classified yet (a reference alternative, never parsed)
  answers as it will once it has. An operator's handle is the operator the choice built when
  it is classified, else the sequence it is made from, named as the operator.
- **One rule set for tokens and patterns** (`introspect/select.rs`). v12 writes each rule
  twice (`getTokens`/`getPatterns`, `…After`, `getNext…`); the copies differ only in what a
  leaf contributes, in how `!x` and a cut turn "what follows me" into their own answer, and in
  how a list of next patterns becomes the answer. `Select` says those three things for
  `Tokens` and for `Patterns`; every per-kind rule is written once.
- **The repeat facade.** v12 wraps each repeat in a `Repeat` whose only child is the
  `InfiniteRepeat`/`FiniteRepeat` that works. The Rust `Repeat` answers as the inner repeat
  does with the facade as its parent, which is what every query climbing from inside the
  repeat sees in v12 (after the child: the divider, or the child again, and what follows the
  repeat). Asking the facade itself `getTokensAfter(x)` ignores `x` and answers what follows
  the facade; no such second pattern exists here (a deviation below).
- **`find` is each pattern's own `find`, then `findPattern`.** A literal, regex, take-until or
  reference finds nothing, not even itself; a context runs its child's own `find` (so a
  literal child finds nothing); everything else searches itself: children first, depth first,
  never into a reference. A choice's children are its classified children, which v12 searches
  once built. A bounded repeat's search stops after its first child and divider slots: later
  slots hold the same patterns, and `{1,1000000}` has a million.
- **`is_equal`** is v12's: the same type with equal children in order; a literal compares its
  token, a regex its source, a reference its name. A choice compares its alternatives as given
  (v12 compares classified children once built, which follow from them). A bounded repeat's
  children are its slots, so its bound, divider and trimming count; `min` doesn't, as in v12.
  `Pattern::type_name` is v12's `type` (`"expression"` for a choice).
- **`clone_named`** replaces `renamed` (v12 `clone(name)`; one name for one operation).
- **`Tokenizable`** is a trait (`tokens`, `set_tokens`) that `Regex`, `TakeUntil` and `Custom`
  implement, so `Literal` and `Sequence` can't be given tokens at compile time; on a `Pattern`,
  `tokenizable_mut()` is `None` for every other kind. The regex's and take-until's inherent
  `tokens`/`set_tokens` moved onto it, and `Custom` got a token list (a v12 custom pattern
  writes its own `getTokens`; a Rust one is told its tokens, like a regex).
- **`generate_error_message(pattern, result)`** prints v12's text exactly, quirks included,
  because callers match on it: with no leaf match (or one ending at 0), line 1, column 1 and
  the root's tokens joined with `", "`; after one, its next tokens joined with `","` (a JS
  array in a template string), and the line and column of the text up to ONE UTF-16 UNIT PAST
  the match (v12's `substring` is inclusive): a newline right after the match starts line
  n+1 at column 0, and an emoji right after it counts one unit. Suggestions are trimmed and
  blanks dropped. It needs the text, so `ParseResult` keeps it (`ParseResult::text`).
- **A name that resolves to nothing contributes nothing.** v12 throws "Couldn't find". A
  query is a question, not a parse, and `check`/`exec` already report the name as data (§11).
- **A query is bounded twice, by counts, never by time.** A grammar whose expansion never ends
  (`a = "x"? a`: only a FIRST child is checked for recursion) makes v12 recurse until the JS
  engine throws `RangeError`, and the same recursion would abort a Rust process. The chain
  stops growing `MAX_DEPTH` (256) levels below the place the query was asked about, which
  bounds the stack (checked on a 2 MB test thread). The limit counts from that place, not from
  the root, and the walk down the asked-about path is bounded by the path alone (fix round 1):
  a leaf match's path comes from a parse, which nests as deep as `Limits::max_depth` allows
  (90 parentheses put a leaf 271 steps down), and a limit counted from the root had emptied
  `next_tokens` and the error message's hint there. The deepest default parse (340 levels, a
  1021-step path) walks, climbs to the root and meets a never-ending expansion on a 2 MB
  thread; a caller that raises `max_depth` gives the query the stack it gives the parse.
  *The path is bounded too (fix round 2).* A path is a public argument, and every step adds a
  link to the query's chain, which the climb recurses up and the drop recurses down: a
  300 000-step path through the parentheses grammar aborted the process. A query follows a
  path of at most `Limits::max_depth` steps and treats a longer one as leading nowhere (`at`
  is `None`, the lists are empty). That refuses no real path: every step below the root enters
  a composite, which takes a parse frame, or ends at a leaf, so a parse under the same limits
  can't make a longer one (measured: the deepest default parse of four nesting grammars,
  through references, contexts and optionals, puts its leaf 1021 steps down). `Introspection::
  new` uses `Limits::default()`, `Introspection::with_limits` takes the parse's, and
  `ParseResult` keeps the limits it ran under (`limits()`) so `generate_error_message` follows
  an `exec_with` path. *Rejected:* an iterative `Drop` and an iterative climb alone would
  remove the overflow but not the work: each `is_recursive` check scans the chain and each
  context on the path copies the frames above it, so the work grows with the square of the
  path's length. A count tied to the parse's own limit bounds both and is the same on every
  machine.
  Depth doesn't bound the work: an expansion that branches (`a = !"z" a`, `a = "x"?
  ("y"? a)? a`) makes twice the places per level, and ran for minutes before the second
  bound: a query makes at most `BUDGET` (100 000) places, then answers with what it found.
  The budget is per query, reset at every public entry. Every query over the grammars of
  v12's tests made at most 31 places (measured), so neither bound fires on a real grammar,
  however deep the input.
- **Checked against v12** by running both (a TS runner against v12's `src` and a Rust one,
  reading one JSON spec) on 34 grammars: every v12 test grammar here, references to
  operators, renamed references, contexts found by their own name, nested contexts, homes,
  roots parsed before the query, cuts and lookaheads in every position, every repeat kind,
  blocks, take-until, literal sets, multi-line and emoji text. For every place reachable to
  depth 6: tokens, patterns (names and paths), next tokens and patterns, and tokens and
  patterns after each child (432 places); `find` for every name; the root's tokens; and
  `generate_error_message` on 115 inputs. Same answers except the deviations below. The tests
  pin v12's exact answers where v12's own tests only check `toContain` or a length. A
  mutation pass removed or bent each mechanism above (the two recursion guards, the
  continuation past optionals and lookaheads, each sibling and repeat rule, each choice role,
  `!x`/cut answering what follows, home binding at each place it applies, resolved instances
  first, the readied scope, the frames of contexts and operator parts, renaming, each `find`
  rule, each `is_equal` rule, each message quirk, the depth limit, the budget and its reset);
  each made a test fail.
- **Deviations from v12:**
  - The repeat facade (above): `Repeat.test.ts`'s 'Finite Repeat Get Tokens', 'Get Tokens
    After' and 'Repeat Get Patterns' ask the facade for what follows `number`, a child it
    doesn't have, and v12 answers what follows the facade (`[]`, `["B"]`, `[]`). Here the
    repeat that owns `number` answers (`["1"]`, `["1", "B"]`, `[number]`), which is v12's
    `number.getNextTokens()`. Pinned in `tests/introspect_repeat.rs`.
  - v12's `RightAssociated` doesn't make itself its child's parent, so a marked atom asked
    for what follows it answers `[]` there (and an error message after one has an empty
    hint). The marker is no step here, so the atom answers what follows the marker. A marked
    operator is unwrapped into its operator in both. Pinned in `tests/introspect_choice.rs`.
  - v12's `TakeUntil` doesn't parent its terminator (§15): its handle's path stops at the
    terminator there; here it is the take-until's child 0.
  - Unknown names contribute nothing; v12 throws. Never-ending expansions answer within the
    two bounds; v12 throws `RangeError`. Pinned in `tests/introspect_reference.rs` and
    `tests/introspect_sequence.rs`.
  - Home binding (§11) applies to queries as it does to the parse: a target found in the
    implicit scope resolves its names among the contexts around its definition, so a
    reference inside it to the enclosing context's own name finds that context, where v12
    resolves from where the reference is used.
  - A context found by its own name (v12 `getPatternWithinContext`) is built at lookup and is
    in no tree, so a handle at its place holds its child (named as the context). v12 hands out
    the context.
  - v12's choice has no children until its first parse or `build()`, so `find` and `isEqual`
    on an unbuilt choice see none; here they always see them (as the implicit scope does,
    §11). v12's tests build first.
  - A marked atom's recursion check uses the atom's id; v12's atom is the marker clone, with
    the marker's id. Only a marked atom that leads back into its own choice could tell.

## 17. AutoComplete *(milestone 9)*

- **v12's algorithm, step for step** (`src/intellisense`): a recording parse of the text; the
  options from each leaf match (what may follow it, `next_patterns` by its path, then its own
  suggestions trimmed by the text it matched) and from every recorded error whose `last` is at
  the end of the text (its pattern's suggestions trimmed by the text from the error's start);
  each list placed against the text (`findMatchIndex`), cut to the options that start furthest
  and sorted; then merged, first option per text, cut to the furthest again. A composite
  pattern's suggestions are its leaf patterns' tokens and custom tokens (custom first), deduped
  by text plus the sorted segment names; a greedy name joins each with every suggestion of what
  may follow it. The three error cases (options and no cursor error: from the earliest option
  to the end; no options but a tree: from the tree's end; a cursor error: the further of the
  last match's end and the furthest error's `last`, compared as v12 compares an exclusive end
  with an inclusive last), and empty text (the root's suggestions, error `0..0`, no parse), are
  v12's. Its own errors name the root, with no path.
- **The parse is `parse`, not `exec`** (`exec::parse_recording`): v12 calls
  `pattern.parse(cursor)`, so no summary error is recorded, the cursor's error is what the parse
  left, and the tree is kept whether or not it spans the text (`Suggestion::tree`, v12's
  `ast`). References are readied as `exec` readies them, and `AutoComplete::new` runs
  `Pattern::check` (a name that resolves to nothing is a build error, §11; v12 throws when a
  parse reaches it).
- **Patterns are named by places.** A segment's `pattern` is a `SegmentPattern`: the path from
  the root AutoComplete was built with, the name and the id (v12 hands out the pattern object,
  which knows its parents; §2). Every query is a milestone-8 query by path, so a leaf match's
  path from the parse is where autocomplete starts. A leaf's suggestions are read off the
  pattern at the handle (`select::leaf_tokens`, the one rule both use), not asked by path: a
  leaf's `getPatterns` is itself, and asking would walk its path from the root once per leaf
  (a choice of 20 000 keywords takes 0.4 s in a debug build this way).
- **Positions.** Byte offsets (§5). v12 selects the errors whose `lastIndex` is `length - 1`,
  the last UTF-16 unit; here `last` at the start of the last `char`, which is what a `Literal`
  that ran out of text records in both, on any text (an emoji's low surrogate there, its first
  byte here). v12's `substring(start, last)` is inclusive, so an error's text runs to the end of
  the text and a match's prefix to its end. `findMatchIndex` compares whole `char`s, so an
  option never starts inside a character (v12 can split a surrogate pair when two emoji share
  their high surrogate).
- **Sorting: JS `localeCompare`, reproduced** (`intellisense/collate.rs`, `locale_compare`).
  v12 sorts with ICU's default collation, which is not code-point order: case is a tie-break
  (`a < A < b`), punctuation and symbols sort before digits and digits before letters in ICU's
  own order (`_ - , ; : ! ? . ' " ( ) [ ] { } @ * / \ & # % \` ^ + < = > | ~ $`), accents
  outrank case (`e < E < é`), a word sorts before a longer word it begins (`á < a b`), and
  control characters are ignored. Here: the UCA's three levels (base character, accents as
  separate elements, case), with weights generated once from Node 24: every ASCII character;
  the Latin letters of U+00C0–U+017F, with their combining marks' secondary order, stroke
  letters (`ø`, `đ`, `ł`) placed among them, and `æ`, `œ`, `ĳ`, `ß` as two letters set apart
  above any accent; the dotless `ı` between `i` and `j`; the no-break space; the soft hyphen and
  zero-width format characters ignored. Anything else sorts after every letter, by code point
  (an approximation). *Checked:* 3 000 random strings of 1–4 characters from those sets, sorted
  by both, for 10 seeds: the same order. `tests/autocomplete.rs` pins Node's order for a fixed
  list. *Rejected:* `icu_collator` (megabytes of data in a wasm module that is mostly regex
  tables already, §6, for characters no grammar here offers); byte order (every capital before
  every lowercase letter, `"Z" < "a"`).
- **Dedupe is chosen once.** `disable_dedupe` picks `Dedupe::Off` or `Dedupe::ByKey` in the
  constructor. v12 also dedupes each match's composites and the error options; that step is left
  out, because the merge that follows keeps only the first option per text, which drops
  everything it would (a mutant that removed it survived every test, and no input can tell).
- **Greedy joins are bounded, by counts.** v12 recurses without limit: a greedy pattern whose
  continuation leads back to it (a greedy child of a repeat; `list = item sep list?` through a
  reference) recurses until it throws `RangeError`. Joins nest at most `MAX_GREEDY_DEPTH` (32)
  deep and the innermost offers nothing, so the whole chain does, as a greedy pattern with
  nothing after it does in v12. Joins that branch (`item = "a" | "b"`, both greedy, recursing)
  make 2^32 places before that depth, so the places expanded inside joins are also counted:
  past `BUDGET` (10 000) per composite suggestion a place offers nothing. Only places inside a
  join count; outside one, a pattern expands to its leaves once each, which is linear in the
  grammar, so a long keyword list is never cut short. Every v12 test joins a few dozen places.
- **Checked against v12** by running both (a TS runner against v12's `src` and a scratch Rust
  binary, reading one JSON spec of grammars, options and inputs, with shared instances where
  v12's tests share them) on 57 grammars and 364 inputs, many as every prefix of a full input:
  every AutoComplete.test.ts grammar, the Expression.test.ts expression and tail grammars,
  mutual recursion, a Pratt choice in a statement, blocks (content, optional content, wildcard),
  take-until, contexts, `!x`, cuts, bounded repeats with and without trimming, multi-byte and
  emoji text, CRLF, custom tokens through references, greedy chains, and the sort. Same
  completeness, options (text, start, segment texts and names, order), error, error index and
  tree value, except the deviations below. A mutation pass removed or bent each mechanism (the
  merge, both furthest cuts, the error filter, each error case, the root fallback, the extending
  suggestions, the trims and the segment filter, dedupe by key, custom tokens and their order,
  the greedy check, the depth limit, the budget, each collation level, the digraph and no-break
  weights, the ignorables, the tie rule's floor); each made a test fail or, for the budget,
  never finish. Fix round 1 found two weaker mutants that the first pass missed. One keeps
  every error rather than only those ending at the last character, which turns custom tokens
  on a sequence that failed earlier into options. The other keeps an option that starts before
  the furthest when it comes after it, which adds `ohn` to `Ja` and moves the flag grammar's
  error back to the space. Both are now pinned by v12-checked tests in `tests/autocomplete.rs`.
- **Not ported:** `suggestForWithCursor` (it is `suggest_for` on a cursor's text) and the
  `cursor` field (the parse's history is not kept on the `Suggestion`; the tree is). The wasm
  layer gets AutoComplete with UTF-16 offsets in a later milestone.
- **Deviations from v12:**
  - A greedy join that never ends offers nothing (above); v12 throws `RangeError`. Pinned in
    `tests/autocomplete.rs`.
  - Unknown names fail `AutoComplete::new` (above); v12 throws when a parse or query reaches
    one.
  - Characters outside the collation tables sort by code point after every letter; v12 sorts
    them as ICU does.
  - Positions are bytes and options never start inside a character (above).
  - The milestone-8 deviations apply to the queries autocomplete asks (the repeat facade, a
    marked atom, unknown names, home binding).

## 18. The grammar builder *(milestone 10)*

- **Layout** (`src/grammar/`): `cpat.rs` (the hand-written grammar), `mod.rs` (`Grammar::
  parse_string`, the syntax parse, the removed-`<|>` check), `builder.rs` (the walk over the
  statements, the rule table, the record), `build.rs` (one pattern per syntax node, dispatched on
  the closed `syntax::Syntax` enum), `repeat.rs`, `block.rs`, `checks.rs` (unknown names, alias
  resolution, shadowed literals), `escapes.rs`, `json.rs` (`JSON.stringify` of a string),
  `anonymous.rs`, `patterns.rs` (`Patterns`, the record), `error.rs` (`GrammarError`).
- **The hand-written grammar matches v12's trees exactly.** Checked by running both on the 11
  agent-harness grammars (436 KB of trees, byte offsets on both sides) and on 46 snippets that
  use every construct and every syntax error path: identical trees and identical
  `generate_error_message` text.
- **The builder is v12's `_buildPatterns`, step for step**: rules in order; a name defined
  earlier is copied in (a clone that keeps its id), a later one is a `Reference` noted as
  pending; then unknown names, then aliases, then shadowed literals, each the first failure in
  v12's order. Literal and regex parts in anonymous position are named by their content; `!p`,
  `p?` are named `not-<inner>`, `optional-<inner>`; `(p)*` is `Optional(name, Repeat(
  "inner-optional-<name>"))`; block delimiters are literals `open` and `close`; `~/` inserts
  `Cut::default()`. Each `GrammarError` prints v12's message character for character, the
  shadowed literal quoted with `JSON.stringify`, the removed `<|>` located by its outermost
  node (v12's `Node.find` lets an ancestor that also matches win, so `a <|> b <|> c` is named
  whole) with its column in UTF-16 units. A syntax error's message comes from a recording parse,
  as in v12; it is run only after the ordinary parse failed, since both find the same leaf match.
- **The record shares one table.** v12 wraps every rule in `Context(name, p, all other rules)`.
  Here every context's scope holds one shared `Rc` of the definitions (`scope::Definitions`,
  `Context::sharing`): the shell grammar has 345 rules, and a copy of every other rule per rule
  would be 119 000 clones of whole rule trees. The shared table holds each rule's own definition
  too; a context answers its own name before its table, so a lookup never reaches it. (The only
  visible difference: `patterns_within_context` lists the context's own rule as well; v12
  leaves it out.) As in v12, definitions are keyed by each pattern's own name, so a rule whose
  pattern has another name (`x = "a"?` is named `optional-a`) can't be found by a reference to
  `x`; see the known limitation below.
- **A context found by its own name is built only when needed** (`scope::Found::master`). Every
  lookup that hit a context's own name used to copy the context's whole child at once. Over a
  grammar that names its entry rule from inside (the shell's `script`, 16 600 patterns), the
  up-front reference check took 5.9 s for that copying alone (measured, release build); it now
  makes the copy only for a target it walks for the first time (33 ms), and a parse makes it once
  per reference instance, as before.
- **Classification stays at first parse.** The builder could classify each choice with the
  definitions it holds (`Choice::classify_with`), but which pattern a name means depends on the
  context running: a rule's context answers its own name with itself (an atom), while every
  other context finds the rule's definition (which may be an operator sequence). So `x = y |
  "a"`, `y = x + "!"` classifies differently run from `x` and from `y`, in v12 and here; only
  the parse-time scopes give v12's answer. The cost is §8's: each instance with reference
  alternatives classifies on its first parse.
- **Unreachable shapes.** A node the builder has no rule for (`UnbuildableNode`), an empty group
  and a lone block delimiter are `GrammarError`s with v12's text, though the cpat grammar never
  produces them where the builder looks; unit tests build them from hand-made nodes. An operator
  node without its operand (v12's "… missing inner pattern" throws) is an `expect`: the grammar
  makes every operator sequence with its operand.
- **The self-host test** (`tests/grammar_fixtures.rs`): `fixtures/cpat.cpat` is cpat's grammar
  in cpat; built by the builder, it parses cpat.cpat itself, the 11 harness grammars and seven
  snippets covering the rest of the syntax to the same trees as `cpat_grammar()`, and refuses
  the same bad inputs at the same place. Punctuation is written inline so its nodes are named
  by their text, as v12 names them. The one name a grammar file can't give is the comma's: v12
  names a regex `,`, and a rule name must be a word, so the file calls it `comma` and the test
  maps it.
- **The harness grammars** (`tests/fixtures/harness/`, copied at agent-harness bef209c) all build,
  with v12's rule counts, and 44 inputs through their entry rules give the trees v12 gives
  (`fixtures/harness_smoke.tsv`, written by v12). Building jq's grammar found a milestone-1 bug:
  `fancy-regex` refuses the `(?-u:\b)` spelling of an ASCII word boundary, so a regex with a
  lookahead and `\b` didn't build. The translation now has a dialect per engine, and for the
  backtracking engine `\b`/`\B` are spelled with lookaround over `[0-9A-Za-z_]`, which is what
  JS's `\b` means (`tests/regex.rs`).
- **Checked against v12** with a TS runner over v12's `src` and a scratch Rust binary on the same
  inputs: the hand-written grammar's trees and messages (above); the builder's errors on 41
  grammars (identical except construction errors, below); every harness entry rule; and each
  ported test's values (the greedy columns, anonymous node names, error patterns, rule order).
  The cut tests' error counts are 6, 9, 12, … with the cut and at least doubling without it; v12
  counts 5, 8, 11, … calls to `recordErrorAt`, and records the summary error through
  `recordSummaryErrorAt`, which is the one more here (`errors()` lists both).
- **How deeply a grammar may nest** (fix round 1, `grammar/depth.rs`, `tests/grammar_depth.rs`).
  Grammar text is input, and two shapes of it ran away:
  - *Parentheses double the build.* `patternExpr` tries `( p , divider )+` before `( p )`, as in
    v12, so every `(` parses its inside twice, and each reference reached keeps its own clone of
    what it names (§1, §3): every level of parentheses doubled the clones. 14 levels took 6 GB,
    100 never finished; v12 is the same shape (16 levels, 4.6 GB). Reordering the alternatives
    would change what `(a)+` means, merging them would change the trees (which must stay v12's),
    and sharing instances between references is §1's settled question. So the cpat grammar reads
    a grammar's text under `Limits { max_depth: 38 }`: 8 composites for a statement and 3 per
    level of `( … )`, `[…] … […]` or `?->|`, which is 10 levels of parentheses (about 40 MB at
    most). Past that the parse halts at once (§11) and the build fails with
    `GrammarError::NestedTooDeep` at the line and column where the level too many begins. The
    deepest text in the fixture corpus needs 13 of the 38.
  - *Operator runs overflow the builder.* Prefix and postfix operators (`!!!…`, `…???`,
    `… right right`) are a loop in the Pratt choice, not nesting in the parse, so the text is
    fine, but the builder recurses once per operator: 8000 `!` overflowed the 8 MB main stack, a
    process abort. The builder now refuses a pattern built inside 256 others with the same
    error at the node's position; 256 levels is a small part of a 1 MB wasm stack in a debug
    build.
  - Two smaller fixes found on the way. Each regex is compiled once and shared (`Rc`) by every
    clone of its pattern: a clone of a `regex::Regex` starts its own match cache, and those
    caches were seven-eighths of a clone's memory. And `parse_string` parses with a never-parsed
    copy of the thread's cpat grammar (cheap now, since the regexes are shared), so the clones
    one build makes are freed with it instead of living as long as the thread.
- **Deviations from v12:**
  - Nesting deeper than the limits above is `GrammarError::NestedTooDeep` ("[Invalid Grammar]
    the grammar nests too deeply at line L, column C; give the inner expression a rule of its
    own."). v12 has no such error: it builds 11 to about 15 levels of parentheses (with memory
    doubling per level) and 256 `!` in a row, and throws a `RangeError` for 8000.
  - A repeat's divider and quantifier are its own children. v12 searches the repeat's whole
    subtree (`node.find`), so a repeat whose item is itself a repeat took the item's divider and
    quantifier: `x = (("a", ",")+, ";"){2}` built a `,`-divided one-or-more and refused
    `a,a;a`. None of the harness grammars has such a repeat (checked). Pinned in
    `tests/grammar.rs`.
  - A pattern that can't be constructed (an empty literal, an anchored or invalid regex) is
    `GrammarError::Pattern` with cpat-rs's own message, which names the pattern. v12 passes on
    its constructors' text ("Value Cannot be empty.", "Invalid Arguments: …").
  - A `\u` escape that is half of a surrogate pair with no partner decodes to U+FFFD: a Rust
    string can't hold a lone surrogate. Two `\u` escapes that form a pair are one character, as
    in v12.
  - The unknown-name check of a hand-built reference is up front (§11), so v12's "a hand-built
    Reference stays lazy" is `ParseResult::pattern_error` here.
- **Known limitation, kept from v12:** a reference from an earlier rule to a rule whose pattern
  is named otherwise (`y = x + "b"`, then `x = "a"?`) names nothing in scope: v12 throws
  "Couldn't find 'x'" at first parse, and `exec` reports it up front here. Defined first, the
  rule is copied in and works. Pinned in `tests/grammar.rs`.
- **Not ported here:** the async API (§19). The wasm layer gets grammars in a later milestone.

## 19. Imports, params and decorators *(milestone 11)*

- **Layout** (`src/grammar/`): `imports.rs` (the `ImportResolver` port, `GrammarFile`, and the
  `InMemoryImports` adapter; a closure is a resolver too), `options.rs` (`GrammarOptions`:
  `resolver`, `origin_resource`, `params`, `decorators`), `setup.rs` (what one build is given,
  and what a build made for another shares with it), `import.rs` (imports and `use params`, in
  statement order before any rule), `decorators.rs` (the registry), `tokens.rs` (`@tokens`),
  `decorate.rs` (the walk back from a rule), `template.rs` and `camel_case.rs` (`patterns`,
  `patterns_with`; `patterns!` is in `lib.rs`).
- **One synchronous resolver.** v12 has `resolveImport` (async) and `resolveImportSync`; the core
  has one `resolve(resource, origin)`, called while building. A host that loads grammars
  asynchronously fetches them first and answers from memory (`InMemoryImports`). v12's async
  tests (`Grammar.parse`, `Grammar.import`) are synchronous here. *Rejected:* an async build in
  the core (it would put an executor below the wasm layer for no case the prefetch can't serve).
- **v12's build, step for step.** A build is given its params, its origin, the merged decorators,
  the anonymous count and, for a `with params` block, the importer's rule names (v12
  `_outerNames`). Imports and `use params` run in statement order; a `with params` block is
  built first, as a grammar of its own that sees the importer's imports and params; the
  imported grammar is built with those params, its own origin and the same count. A name is
  looked up in the rules built so far, then the imports, then the params (v12 `_getPattern`), so
  a rule used before a same-named rule redefines it gets the import. An alias names one of this
  grammar's rules only when no import or param has that name (`space = space` passes a param
  through, it is no cycle). An imported alias is a clone under the alias name. A `use params`
  default is what an import brought in, else a reference checked with the unknown names. Each
  import's failure after loading (the imported grammar's error, a name taken twice, a name it
  doesn't define) is wrapped as v12 wraps it, `Failed loading expression from: "…". Error
  details: "…"`, so a chain of imports nests; the resolver's own error and "No import resolver
  supplied." are passed on unwrapped, as in v12. Checked against v12 on 75 grammars (every
  ported test's, and the edge cases above): identical results, values and messages, except the
  import cycle below.
- **The resolver's `origin`** is the resource the resolver answered with for the importing
  grammar (not the name it was asked for), `origin_resource` for the top-level text, and the
  importer's own origin for a `with params` block, as in v12. An empty origin is none (v12's
  `originResource || null`).
- **Decorators return the pattern to keep** (§9), where v12's change it in place. They run
  nearest first (v12 walks back from the rule over comments and blank lines). The registry keeps
  registration order for the unknown-decorator message, and the built-in `@tokens` is merged as
  v12 merges it (`{ ...decorators, ...defaultDecorators }`): it can't be replaced, and one the
  caller also registered keeps the caller's place in the list. A forward alias is decorated
  once, when it is resolved; an export is never decorated. `@tokens` finds its target by
  capability (`Pattern::tokenizable_mut`), so it takes a regex, a take-until or a custom pattern,
  and names anything else by v12's `type` ("a expression" for a choice, "a context" for an
  imported rule).
- **A decorator must keep the pattern's name** (`GrammarError::DecoratorRenamed`). Rules are
  found by their pattern's name (§18), so a renamed rule would silently stop resolving. v12 can't
  reach this: its decorators can't replace the pattern.
- **v12 quirk kept:** `@name(true)` and `@name(false)` pass no argument. The cpat grammar's
  `jsonBoolean` is a choice of two literals, whose node is named after the literal, so v12's
  search for a JSON node finds none. Pinned in `tests/grammar_decorators.rs`.
- **Deviations from v12:**
  - A grammar that imports itself, directly or through others, is `GrammarError::ImportCycle`
    ("[Invalid Grammar] import cycle `a` -> `b` -> `a`: …"). v12 recurses until the JS stack
    overflows and wraps the `RangeError` once per level; in Rust that would be a process abort.
    A cycle is a grammar met again along the chain of importers with the same text *and* the same
    origin (the resource the resolver answered with, empty meaning none): what a build imports
    depends only on those two, so a repeat is certain to recur forever. The resource alone is not
    enough (fix round 1): v12's own tests answer every import as `"r1"`, so a resolver may answer
    different grammars under one name, or under the empty name, and v12 builds those. A resolver
    that answers with new text or a new name on each import hides a cycle from that check, so
    imports nested more than 64 deep are `GrammarError::ImportsTooDeep` (`MAX_IMPORT_DEPTH`;
    every level counts, named or not; each holds only a builder, since the cpat parse has
    returned before imports load). A grammar imported twice along different paths is no cycle.
  - A decorator argument that is cpat but not JSON (`"\x41"`, a leading zero) is
    `GrammarError::DecoratorArgument` with `serde_json`'s message; v12's `JSON.parse` throws V8's
    message, unprefixed.
  - `patterns` returns a `HashMap`, so its keys have no order; v12's object keeps the rules'.
    Two rule names that meet in camelCase keep the later rule, as in v12.

## 20. The editable tree *(milestone 12)*

- **Layout** (`src/ast/`): `editable.rs` (`EditableTree`, `NodeRef`, making nodes, reads),
  `splice.rs` (child edits), `search.rs` (walks, finds, equality, JSON on the editable tree),
  `rewrite.rs` (`compact`, `clone_node`, `normalize`), `transform.rs` (`Visitors`),
  `by_name.rs` (`ast::compact`, `ast::remove`), `from_tree.rs` (`Tree::to_mut`),
  `read_only.rs` (the read-only `Node`'s share), and the parts both trees share through one
  crate-private `AstView` trait: `walk.rs`, `json.rs`, `equality.rs`, `cycle_free.rs`,
  `view.rs` (the trait and v12's `toString`).
- **An arena of slots, a `NodeRef` per node.** Each slot owns its type and name
  (`Cow<'static, str>`: a parse node borrows the interned name), its own value
  (`Cow<'t, str>`: a parse leaf borrows its slice), its half-open range, parent and children.
  Methods take the `NodeRef` they act on, where v12's take `this`, so a visitor gets `(&mut
  EditableTree, NodeRef)` and may edit as it walks. A removed node stays in the arena,
  parentless, as a v12 node lives on while something holds it; nothing is ever freed before the
  tree is. *Rejected:* `Rc<RefCell<Node>>` with weak parents (a borrow per read, cycles to
  manage, and a recursive drop on deep trees); a handle that borrows the tree (it can't be
  held across an edit).
- **Ids.** v12 numbers every node from one process-wide counter. Here an id is the read-only
  tree's arena index, which `to_mut` keeps, so the read-only JSON and the editable JSON are
  identical; new nodes count on from the arena's length. The tests that compare with v12's JSON
  for a parse mask ids, because v12's depend on everything parsed earlier in the process.
- **v12's semantics, quirks included,** each pinned by a test (`tests/ast.rs`): `walk_up` stops
  visiting siblings after a `false` but still visits every ancestor, so `find` lets the
  outermost matching ancestor of the first match win (v12's `find(n =>
  n.name.startsWith("x"))` returns the parent); `walk_up`/`walk_down` read a node's children
  before visiting it and `walk_breadth_first` after, so `compact` by name skips what it
  compacted; adopting a node doesn't take it out of its old parent's list; `insert_before` adopts
  even when the reference node isn't a child; `splice_children` clamps as `Array.splice` does;
  `clone_node` makes children before their parent; `normalize` clamps a last index at 0.
- **The read-only `find` changed** to that semantics. It returned the first match children
  first; for a predicate that matches a node and its ancestor the answer is now the ancestor,
  as in v12 (`tests/tree.rs` pinned the old answer and now pins v12's, checked against v12).
  No code in the crates relied on the old order.
- **Parse nodes' own values.** A leaf's own value is its slice; a parent's is "". v12's
  sequences, repeats and blocks also keep "", so a parent stripped of every child reads "" in
  both. *Deviation:* v12's expression nodes are made by `createNode`, which snapshots the join of
  whatever children (and placeholders) they had then; here they keep "" too. It shows only after
  every child of an expression node is removed without compacting it.
- **Lengths are bytes.** `normalize` lays leaves end to end by `value.len()`, bytes, as every
  position is (§5a); v12 counts UTF-16 units. They agree on ASCII.
- **The JSON is written, not built.** `to_json(space)` writes v12's `JSON.stringify` output
  straight from the tree: the keys in v12's order, ids as strings, `[]` for no children, indent
  capped at 10, and `JSON.stringify`'s escaping (quotes, backslashes and control characters
  only; lowercase `\u00xx`). The escaping and indents are compared byte for byte with files v12
  wrote (`tests/fixtures/ast/`).
- **Nothing recurses (§7e).** The walks, JSON, `clone_node`, `normalize`, `transform`,
  `is_equal`, `to_cycle_free_object` and `CycleFreeNode`'s drop and equality keep their own
  stacks; a test runs all of them on a 5,000-level operator chain on a 256 KiB thread, where a
  one-line recursive node count overflows. `is_equal` compares values at leaves only: equal
  children make equal joined values, so this keeps it linear where v12 joins at every level.
  JSON and cycle-free values remain quadratic in depth, as in v12, since each repeats its
  subtree's text.
- **The wasm layer is unchanged.** JS keeps its read-only view (`js/tree.mjs`); an editable tree
  in JS comes with the JS milestone.

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
| Only sequences classify as operators (`b = a \| "d"` inside `a` is an atom) | `tests/expression.rs` |
| A right-associated operator nests right past a tighter one (`a as b + c as d`) | `tests/precedence.rs` |
| Mutually recursive choices (`a = b \| "c"`, `b = a \| "d"`) parse | `tests/expression.rs` |
| Mutually recursive choices autocomplete without overflowing the stack | `tests/autocomplete.rs` |
