# The benchmark

TS clarity-pattern-parser v12 (under Node) vs cpat-rs native (release) vs cpat-wasm (release,
from Node), on the same workloads. A measurement, not a test: nothing here runs in
`cargo test` or `node --test`, and it asserts nothing about time. Timing is the point here,
so it lives outside the test suites.

```sh
node bench/run.mjs                      # everything: ~20 minutes on an M5 Pro
node bench/run.mjs --only 'json|calc'   # workloads whose name matches
node bench/run.mjs --quick              # 3 runs, no warm-up: a smoke run of the harness
node bench/run.mjs --no-build           # reuse target/release/cpat-bench and pkg-node
node bench/run.mjs --out target/bench-x --policy 5,1000,20,4000,2000
```

It needs the v12 checkout next to this repo (`../clarity-pattern-parser-v12`, or `CPAT_V12`),
`cargo` and `wasm-pack` (`~/.cargo/bin` is put on `PATH`), and `npx tsx`. It writes
`target/bench/bench.md` (the report), `results.json` (every sample), `verify.json` (every
engine's signature for every workload), `index.json` and `workloads/` (the generated inputs).
The last committed report is [RESULTS.md](RESULTS.md).

## Steps

1. **Build** `cpat-bench` and `cpat-bench-peak` (this directory's `native/`, a workspace member) and
   `crates/cpat-wasm/pkg-node`, both in release mode (`lto`, `opt-level = 3`, `panic = abort`).
2. **Generate** the workloads (`lib/generate.mjs`, run under tsx with TS v12): one JSON file
   each, holding the grammar text, the `(rule, input)` cases and `reps` (how many times one
   iteration runs the cases). Every input is a pure function of the conformance corpus and a
   fixed seed, so every run and every engine gets the same bytes.
3. **Verify.** Each engine computes every workload's signature (`lib/signature.mjs`,
   `native/src/signature.rs`): for a build, the rule count and a hash of the rule names in
   order, or the error; for each parse, match or not, and on a match the node count and the
   root value's length and hash, on a miss the error's start and last; for each suggestion,
   whether it is complete, `errorAtIndex` and a hash of the options (text and start, in order).
   Positions are UTF-16 offsets. A workload where native or wasm differs from TS v12 is
   reported as a correctness failure and is not timed.
4. **Time** each agreeing workload, one process per engine and workload, engines taking turns
   per workload, under `/usr/bin/time -l` for peak RSS. The policy (`lib/timing.mjs`,
   `native/src/timing.rs`, passed to both as one argument): warm up for at least 3 iterations
   and 500 ms, then time at least 10 iterations and 2 s, at most 1000. The report gives the
   median and p95 (nearest rank), throughput, and the speed-up over TS v12.

## Workloads

| group | what |
|---|---|
| `build/*` | `Grammar.parseString` of each agent-harness grammar (shell, jq, saya, sed, calc, find, glob, date-expr, regex bre/ere/pcre, as the harness's tests build them), v12's TypeScript grammar, and a generated 7 200-rule grammar (249 KB). |
| `parse/*` | Realistic documents: a 1 MB shell script and a 1 MB saya program (the harness's test inputs that v12 matches, joined and repeated), and a 256 KB jq program (the test filters as the values of one object). |
| `tests/*` | Every captured test input for each grammar, matching or not, as one batch. |
| `micro/*` | A 64-literal choice (LiteralSet mode), a 6-regex tokenizer, a flat Pratt chain and 200 nested parentheses, JSON (a small document ×500 and 1 MB), failures at the end of 1 MB of JSON and of shell (the error path), the shell grammar's exponential backtick case, and 1 MB of non-ASCII text with an ASCII twin of the same shape (the grapheme path). |
| `suggest/*` | `AutoComplete.suggestFor` on half-typed shell, jq, saya and calc test inputs, and a cut JSON document. One AutoComplete per rule, made before timing. |
| `noop` | Loads the engine and does nothing: the memory baseline. |

Sizes are set where TS v12 can run them: a flat operator chain of 64 KB overflows v12's
stack (16 KB is used), joining jq filters with `,` would too, and six shell lines of
``echo `…` `` already take v12 0.45 s (eight take 35 s).

## What is compared, and what is unfair

- **Equal output.** TS v12's `exec` builds a tree of `Node` objects. wasm's headline number is
  `pattern.exec(text).ast`, which builds the same `Node` tree in JS from the flat arrays.
  Native's `exec` builds its flat read-only tree and nothing else; byte offsets stay bytes. So
  native does less than the JS engines by design (that's the point of the flat tree), and the
  report says so rather than adding work to it.
- **wasm cost split.** wasm is timed a second time with each step timed by hand (`split`):
  the wasm call plus copying its arrays out, the JS cursor and view, and building the `Node`
  tree. The timer calls add ~0.1 µs per input, noticeable only on batches of tiny inputs.
- **TS runs through tsx**, which transpiles v12's `src` per module with esbuild (current Node
  target). That is close to v12's bundled build at run time; the bundle in v12's `dist/` was
  not used because it is older than the `release/v12` sources.
- **Builds, back to back and released.** The headline runs builds back to back like every
  other workload. wasm-bindgen frees a wasm pattern from a `FinalizationRegistry`, which runs
  only after a collection and on a turn of the event loop, so a synchronous loop of
  `Grammar.parseString` keeps every grammar: wasm memory grows ~10 MB per TypeScript-grammar
  build and each build gets slower (4 ms becomes 60 ms after 40). So builds are also timed
  `released`: a full GC and two event-loop turns between iterations (untimed), for both JS
  engines. The forced GC leaves caches cold, which makes small builds slower there, for TS
  v12 too.
- **Memory.** Peak RSS (`/usr/bin/time -l`) of each timing process includes the runtime (Node
  and tsx for TS; see `noop`) and covers every iteration. Native's RSS overstates its heap:
  macOS's malloc keeps freed pages, so RSS grows with the iteration count while live bytes
  stay flat. So native's heap is also measured exactly by `cpat-bench-peak`, a second binary
  with a counting allocator (kept out of the timed binary): peak live bytes while building,
  and above that during one iteration. wasm's linear memory is read at the end (it never
  shrinks, so it is the high-water mark).
- **Noise.** One machine, not isolated; p95 next to the median shows the spread. Batches of
  tiny inputs run in well under a millisecond per input, where timer resolution and GC
  pauses matter most.
