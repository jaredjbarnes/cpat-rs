# cpat-rs benchmark: TS v12 vs cpat-rs native vs cpat-wasm

2026-10-15. Apple M5 Pro, 64 GiB RAM, macOS 26.4. Node v24.16.0, rustc 1.99.0 (b940084d7 2026-09-28).
Policy per engine and workload, in its own process: warm up ≥ 3 iterations and ≥ 500 ms, then time ≥ 10 iterations and ≥ 2000 ms (at most 1000).
Times are milliseconds per iteration. Speed-up is TS v12's median over the engine's (above 1× the engine is faster).
wasm parses are timed as `exec(text).ast`, the same v12 `Node` tree TS v12's `exec` returns. Every workload runs back to back; builds are also timed releasing what they built (see "Builds that release what they built").

## Summary

Geometric mean of the speed-ups over the workloads in each group (only workloads all three engines agree on).

| group | workloads | native vs TS | wasm vs TS | native range | wasm range |
|---|---:|---:|---:|---:|---:|
| Grammar build | 13 | 3.27× | 1.10× | 1.36–141.88× | 0.19–25.15× |
| Parse: realistic documents | 3 | 4.36× | 1.74× | 3.54–5.64× | 1.43–2.15× |
| Parse: the harness's own test inputs | 12 | 3.36× | 0.78× | 2.72–4.29× | 0.39–1.33× |
| Micro | 11 | 9.39× | 2.54× | 3.57–32.12× | 1.13–9.90× |
| AutoComplete | 5 | 0.85× | 0.06× | 0.43–1.91× | 0.04–0.10× |

| workload | size | TS v12 | native | wasm | native × | wasm × |
|---|---:|---:|---:|---:|---:|---:|
| build/shell | 58.9 KiB | 13.29 | 2.51 | 8.86 | 5.29× | 1.50× |
| build/jq | 22.0 KiB | 5.50 | 1.07 | 5.69 | 5.15× | 0.97× |
| build/saya | 17.5 KiB | 4.61 | 1.26 | 6.11 | 3.65× | 0.75× |
| build/sed | 7.0 KiB | 0.663 | 0.466 | 0.722 | 1.42× | 0.92× |
| build/calc | 6.9 KiB | 1.12 | 0.353 | 1.89 | 3.17× | 0.59× |
| build/find | 6.6 KiB | 0.777 | 0.267 | 0.446 | 2.92× | 1.74× |
| build/glob | 3.4 KiB | 0.302 | 0.213 | 0.286 | 1.41× | 1.05× |
| build/date-expr | 7.2 KiB | 0.889 | 0.624 | 1.08 | 1.42× | 0.82× |
| build/regex-bre | 6.3 KiB | 0.498 | 0.303 | 0.510 | 1.65× | 0.98× |
| build/regex-ere | 2.6 KiB | 0.350 | 0.258 | 0.437 | 1.36× | 0.80× |
| build/regex-pcre | 4.3 KiB | 0.599 | 0.343 | 0.593 | 1.75× | 1.01× |
| build/typescript | 21.9 KiB | 12.19 | 3.68 | 62.88 | 3.31× | 0.19× |
| build/synthetic-1200 | 249.1 KiB | 3436 | 24.22 | 137 | 141.88× | 25.15× |
| parse/shell-1MB | 1.04 MiB | 816 | 231 | 572 | 3.54× | 1.43× |
| parse/saya-1MB | 1.02 MiB | 748 | 180 | 431 | 4.16× | 1.73× |
| parse/jq-256KB | 256.2 KiB | 360 | 63.85 | 168 | 5.64× | 2.15× |
| tests/shell | 130.3 KiB (3754) | 93.85 | 28.08 | 97.30 | 3.34× | 0.96× |
| tests/jq | 28.4 KiB (1130) | 36.16 | 8.44 | 27.46 | 4.29× | 1.32× |
| tests/saya | 70.4 KiB (817) | 50.33 | 13.25 | 37.84 | 3.80× | 1.33× |
| tests/sed | 1.5 KiB (113) | 0.403 | 0.143 | 0.869 | 2.81× | 0.46× |
| tests/calc | 0.7 KiB (96) | 0.645 | 0.226 | 0.943 | 2.85× | 0.68× |
| tests/find | 1.2 KiB (53) | 0.238 | 0.067 | 0.445 | 3.54× | 0.54× |
| tests/glob | 0.3 KiB (28) | 0.164 | 0.046 | 0.295 | 3.59× | 0.55× |
| tests/date-expr | 0.9 KiB (68) | 0.213 | 0.078 | 0.549 | 2.72× | 0.39× |
| tests/regex-bre | 0.5 KiB (85) | 0.757 | 0.206 | 0.865 | 3.68× | 0.87× |
| tests/regex-ere | 0.4 KiB (51) | 0.474 | 0.133 | 0.556 | 3.56× | 0.85× |
| tests/regex-pcre | 0.7 KiB (48) | 1.05 | 0.308 | 0.904 | 3.42× | 1.17× |
| tests/typescript | 5.8 KiB (343) | 5.23 | 1.69 | 5.72 | 3.09× | 0.92× |
| micro/literal-choice-1MB | 1.01 MiB | 126 | 11.21 | 71.34 | 11.25× | 1.77× |
| micro/regex-tokens-1MB | 1.01 MiB | 188 | 25.11 | 116 | 7.47× | 1.61× |
| micro/pratt-flat-16KB | 128.0 KiB (1 ×8) | 41.92 | 4.78 | 16.98 | 8.77× | 2.47× |
| micro/pratt-nested-200 | 53.3 KiB (1 ×50) | 198 | 6.16 | 21.01 | 32.12× | 9.41× |
| micro/json-small | 302.2 KiB (1 ×500) | 40.37 | 6.66 | 35.70 | 6.06× | 1.13× |
| micro/json-1MB | 1.04 MiB | 168 | 21.03 | 110 | 7.99× | 1.52× |
| micro/json-1MB-fail-at-end | 1.04 MiB | 137 | 18.75 | 40.66 | 7.30× | 3.36× |
| micro/shell-1MB-fail-at-end | 1.04 MiB | 830 | 232 | 460 | 3.57× | 1.80× |
| micro/shell-backticks-6 | 0.1 KiB | 384 | 14.41 | 38.74 | 26.62× | 9.90× |
| micro/non-ascii-1MB | 1.02 MiB | 98.17 | 15.45 | 44.97 | 6.35× | 2.18× |
| micro/ascii-twin-1MB | 1.01 MiB | 143 | 14.47 | 74.85 | 9.86× | 1.90× |
| suggest/shell | 0.9 KiB (40) | 3.96 | 3.22 | 40.57 | 1.23× | 0.10× |
| suggest/jq | 0.9 KiB (40) | 3.93 | 8.19 | 64.84 | 0.48× | 0.06× |
| suggest/saya | 2.1 KiB (40) | 5.35 | 5.68 | 71.78 | 0.94× | 0.07× |
| suggest/calc | 0.1 KiB (14) | 0.417 | 0.977 | 6.78 | 0.43× | 0.06× |
| suggest/json | 7.0 KiB (1 ×20) | 0.908 | 0.477 | 21.75 | 1.91× | 0.04× |

Correctness: all three engines agree on every workload.

## Grammar build

| workload | what | engine | median | p95 | runs | throughput | × TS |
|---|---|---|---:|---:|---:|---:|---:|
| build/shell | agent-harness shell.cpat | TS v12 | 13.29 | 14.12 | 151 | 4.33 MiB/s |  |
|  |  | native | 2.51 | 2.77 | 787 | 22.86 MiB/s | 5.29× |
|  |  | wasm | 8.86 | 13.38 | 216 | 6.49 MiB/s | 1.50× |
| build/jq | agent-harness jq.cpat | TS v12 | 5.50 | 6.26 | 356 | 3.92 MiB/s |  |
|  |  | native | 1.07 | 1.13 | 1000 | 20.17 MiB/s | 5.15× |
|  |  | wasm | 5.69 | 8.35 | 345 | 3.79 MiB/s | 0.97× |
| build/saya | agent-harness saya.cpat | TS v12 | 4.61 | 5.46 | 418 | 3.71 MiB/s |  |
|  |  | native | 1.26 | 1.36 | 1000 | 13.57 MiB/s | 3.65× |
|  |  | wasm | 6.11 | 9.17 | 313 | 2.80 MiB/s | 0.75× |
| build/sed | agent-harness sed.cpat | TS v12 | 0.663 | 0.750 | 1000 | 10.37 MiB/s |  |
|  |  | native | 0.466 | 0.503 | 1000 | 14.75 MiB/s | 1.42× |
|  |  | wasm | 0.722 | 1.00 | 1000 | 9.52 MiB/s | 0.92× |
| build/calc | agent-harness calc.cpat | TS v12 | 1.12 | 1.56 | 1000 | 6.00 MiB/s |  |
|  |  | native | 0.353 | 0.371 | 1000 | 18.99 MiB/s | 3.17× |
|  |  | wasm | 1.89 | 3.06 | 1000 | 3.55 MiB/s | 0.59× |
| build/find | agent-harness find.cpat | TS v12 | 0.777 | 1.38 | 1000 | 8.29 MiB/s |  |
|  |  | native | 0.267 | 0.281 | 1000 | 24.18 MiB/s | 2.92× |
|  |  | wasm | 0.446 | 0.780 | 1000 | 14.44 MiB/s | 1.74× |
| build/glob | agent-harness glob.cpat | TS v12 | 0.302 | 0.325 | 1000 | 11.00 MiB/s |  |
|  |  | native | 0.213 | 0.228 | 1000 | 15.55 MiB/s | 1.41× |
|  |  | wasm | 0.286 | 0.629 | 1000 | 11.59 MiB/s | 1.05× |
| build/date-expr | agent-harness date-expr.cpat | TS v12 | 0.889 | 1.33 | 1000 | 7.86 MiB/s |  |
|  |  | native | 0.624 | 0.660 | 1000 | 11.20 MiB/s | 1.42× |
|  |  | wasm | 1.08 | 1.31 | 1000 | 6.47 MiB/s | 0.82× |
| build/regex-bre | agent-harness regex-bre.cpat | TS v12 | 0.498 | 0.541 | 1000 | 12.37 MiB/s |  |
|  |  | native | 0.303 | 0.341 | 1000 | 20.36 MiB/s | 1.65× |
|  |  | wasm | 0.510 | 0.800 | 1000 | 12.07 MiB/s | 0.98× |
| build/regex-ere | agent-harness regex-ere.cpat | TS v12 | 0.350 | 0.389 | 1000 | 7.20 MiB/s |  |
|  |  | native | 0.258 | 0.274 | 1000 | 9.77 MiB/s | 1.36× |
|  |  | wasm | 0.437 | 0.754 | 1000 | 5.77 MiB/s | 0.80× |
| build/regex-pcre | agent-harness regex-pcre.cpat | TS v12 | 0.599 | 0.699 | 1000 | 7.08 MiB/s |  |
|  |  | native | 0.343 | 0.363 | 1000 | 12.37 MiB/s | 1.75× |
|  |  | wasm | 0.593 | 0.928 | 1000 | 7.15 MiB/s | 1.01× |
| build/typescript | v12's TypeScript grammar (typescript.test.ts) | TS v12 | 12.19 | 12.84 | 167 | 1.76 MiB/s |  |
|  |  | native | 3.68 | 3.81 | 542 | 5.82 MiB/s | 3.31× |
|  |  | wasm | 62.88 | 90.49 | 32 | 0.34 MiB/s | 0.19× |
| build/synthetic-1200 | generated: 7 200 rules + an expression | TS v12 | 3436 | 3825 | 10 | 0.07 MiB/s |  |
|  |  | native | 24.22 | 27.59 | 82 | 10.04 MiB/s | 141.88× |
|  |  | wasm | 137 | 202 | 15 | 1.78 MiB/s | 25.15× |

## Parse: realistic documents

| workload | what | engine | median | p95 | runs | throughput | × TS |
|---|---|---|---:|---:|---:|---:|---:|
| parse/shell-1MB | 1 MB shell script (3722 test scripts, repeated) | TS v12 | 816 | 862 | 10 | 1.27 MiB/s |  |
|  |  | native | 231 | 239 | 10 | 4.49 MiB/s | 3.54× |
|  |  | wasm | 572 | 592 | 10 | 1.81 MiB/s | 1.43× |
| parse/saya-1MB | 1 MB saya program (786 test programs, repeated) | TS v12 | 748 | 776 | 10 | 1.37 MiB/s |  |
|  |  | native | 180 | 192 | 12 | 5.69 MiB/s | 4.16× |
|  |  | wasm | 431 | 455 | 10 | 2.37 MiB/s | 1.73× |
| parse/jq-256KB | 256 KB jq program: one object whose values are the 1113 test filters, repeated | TS v12 | 360 | 373 | 10 | 0.69 MiB/s |  |
|  |  | native | 63.85 | 65.70 | 32 | 3.92 MiB/s | 5.64× |
|  |  | wasm | 168 | 177 | 12 | 1.49 MiB/s | 2.15× |

## Parse: the harness's own test inputs

| workload | what | engine | median | p95 | runs | throughput | × TS |
|---|---|---|---:|---:|---:|---:|---:|
| tests/shell | every captured shell test input, matching or not: 3754 | TS v12 | 93.85 | 97.67 | 22 | 1.36 MiB/s |  |
|  |  | native | 28.08 | 29.51 | 71 | 4.53 MiB/s | 3.34× |
|  |  | wasm | 97.30 | 101 | 21 | 1.31 MiB/s | 0.96× |
| tests/jq | every captured jq test input, matching or not: 1130 (1 v12 throws on left out) | TS v12 | 36.16 | 38.11 | 56 | 31,250 parses/s |  |
|  |  | native | 8.44 | 9.20 | 235 | 133,956 parses/s | 4.29× |
|  |  | wasm | 27.46 | 31.45 | 71 | 41,153 parses/s | 1.32× |
| tests/saya | every captured saya test input, matching or not: 817 | TS v12 | 50.33 | 51.41 | 40 | 1.37 MiB/s |  |
|  |  | native | 13.25 | 14.62 | 150 | 5.19 MiB/s | 3.80× |
|  |  | wasm | 37.84 | 42.43 | 52 | 1.82 MiB/s | 1.33× |
| tests/sed | every captured sed test input, matching or not: 113 | TS v12 | 0.403 | 0.515 | 1000 | 280,745 parses/s |  |
|  |  | native | 0.143 | 0.150 | 1000 | 788,374 parses/s | 2.81× |
|  |  | wasm | 0.869 | 0.916 | 1000 | 130,022 parses/s | 0.46× |
| tests/calc | every captured calc test input, matching or not: 96 | TS v12 | 0.645 | 0.686 | 1000 | 148,760 parses/s |  |
|  |  | native | 0.226 | 0.232 | 1000 | 424,154 parses/s | 2.85× |
|  |  | wasm | 0.943 | 0.982 | 1000 | 101,803 parses/s | 0.68× |
| tests/find | every captured find test input, matching or not: 53 | TS v12 | 0.238 | 0.265 | 1000 | 222,650 parses/s |  |
|  |  | native | 0.067 | 0.068 | 1000 | 788,597 parses/s | 3.54× |
|  |  | wasm | 0.445 | 0.475 | 1000 | 119,224 parses/s | 0.54× |
| tests/glob | every captured glob test input, matching or not: 28 | TS v12 | 0.164 | 0.170 | 1000 | 171,079 parses/s |  |
|  |  | native | 0.046 | 0.046 | 1000 | 614,264 parses/s | 3.59× |
|  |  | wasm | 0.295 | 0.330 | 1000 | 94,902 parses/s | 0.55× |
| tests/date-expr | every captured date-expr test input, matching or not: 68 | TS v12 | 0.213 | 0.232 | 1000 | 319,499 parses/s |  |
|  |  | native | 0.078 | 0.082 | 1000 | 867,624 parses/s | 2.72× |
|  |  | wasm | 0.549 | 0.582 | 1000 | 123,768 parses/s | 0.39× |
| tests/regex-bre | every captured regex-bre test input, matching or not: 85 | TS v12 | 0.757 | 1.67 | 1000 | 112,353 parses/s |  |
|  |  | native | 0.206 | 0.211 | 1000 | 412,956 parses/s | 3.68× |
|  |  | wasm | 0.865 | 0.907 | 1000 | 98,242 parses/s | 0.87× |
| tests/regex-ere | every captured regex-ere test input, matching or not: 51 | TS v12 | 0.474 | 0.523 | 1000 | 107,671 parses/s |  |
|  |  | native | 0.133 | 0.140 | 1000 | 383,459 parses/s | 3.56× |
|  |  | wasm | 0.556 | 0.595 | 1000 | 91,740 parses/s | 0.85× |
| tests/regex-pcre | every captured regex-pcre test input, matching or not: 48 | TS v12 | 1.05 | 1.66 | 1000 | 45,544 parses/s |  |
|  |  | native | 0.308 | 0.324 | 1000 | 155,929 parses/s | 3.42× |
|  |  | wasm | 0.904 | 0.950 | 1000 | 53,100 parses/s | 1.17× |
| tests/typescript | every captured typescript test input, matching or not: 343 | TS v12 | 5.23 | 6.75 | 361 | 65,545 parses/s |  |
|  |  | native | 1.69 | 1.77 | 1000 | 202,599 parses/s | 3.09× |
|  |  | wasm | 5.72 | 8.51 | 336 | 59,996 parses/s | 0.92× |

## Micro

| workload | what | engine | median | p95 | runs | throughput | × TS |
|---|---|---|---:|---:|---:|---:|---:|
| micro/literal-choice-1MB | 1 MB of JS keywords: a 64-literal choice in a divided repeat | TS v12 | 126 | 140 | 16 | 7.99 MiB/s |  |
|  |  | native | 11.21 | 11.65 | 178 | 89.81 MiB/s | 11.25× |
|  |  | wasm | 71.34 | 87.30 | 28 | 14.11 MiB/s | 1.77× |
| micro/regex-tokens-1MB | 1 MB of code through a 6-regex token choice | TS v12 | 188 | 203 | 11 | 5.41 MiB/s |  |
|  |  | native | 25.11 | 25.50 | 80 | 40.41 MiB/s | 7.47× |
|  |  | wasm | 116 | 130 | 18 | 8.71 MiB/s | 1.61× |
| micro/pratt-flat-16KB | 16 KB flat operator chain (1+2*3-…) through the Pratt choice, ×8 | TS v12 | 41.92 | 47.05 | 47 | 2.98 MiB/s |  |
|  |  | native | 4.78 | 4.90 | 419 | 26.17 MiB/s | 8.77× |
|  |  | wasm | 16.98 | 42.91 | 91 | 7.36 MiB/s | 2.47× |
| micro/pratt-nested-200 | 200 nested parentheses with an operator each, ×50 | TS v12 | 198 | 202 | 11 | 253 parses/s |  |
|  |  | native | 6.16 | 6.41 | 324 | 8,121 parses/s | 32.12× |
|  |  | wasm | 21.01 | 27.21 | 87 | 2,379 parses/s | 9.41× |
| micro/json-small | a 619-byte JSON document, ×500 | TS v12 | 40.37 | 41.69 | 50 | 7.31 MiB/s |  |
|  |  | native | 6.66 | 6.81 | 300 | 44.31 MiB/s | 6.06× |
|  |  | wasm | 35.70 | 39.25 | 57 | 8.27 MiB/s | 1.13× |
| micro/json-1MB | 1 MB JSON document | TS v12 | 168 | 190 | 12 | 6.18 MiB/s |  |
|  |  | native | 21.03 | 21.71 | 95 | 49.38 MiB/s | 7.99× |
|  |  | wasm | 110 | 130 | 18 | 9.41 MiB/s | 1.52× |
| micro/json-1MB-fail-at-end | 1 MB JSON missing its final ] | TS v12 | 137 | 153 | 15 | 7.59 MiB/s |  |
|  |  | native | 18.75 | 19.16 | 107 | 55.38 MiB/s | 7.30× |
|  |  | wasm | 40.66 | 41.13 | 50 | 25.54 MiB/s | 3.36× |
| micro/shell-1MB-fail-at-end | the 1 MB shell script + an unterminated string | TS v12 | 830 | 861 | 10 | 1.25 MiB/s |  |
|  |  | native | 232 | 234 | 10 | 4.46 MiB/s | 3.57× |
|  |  | wasm | 460 | 469 | 10 | 2.25 MiB/s | 1.80× |
| micro/shell-backticks-6 | 6 lines of echo `echo …`: the shell grammar backtracks exponentially | TS v12 | 384 | 389 | 10 | 0.00 MiB/s |  |
|  |  | native | 14.41 | 15.15 | 138 | 0.01 MiB/s | 26.62× |
|  |  | wasm | 38.74 | 39.13 | 52 | 0.00 MiB/s | 9.90× |
| micro/non-ascii-1MB | 1 MB of mixed-script words, emoji and combining marks | TS v12 | 98.17 | 109 | 21 | 10.36 MiB/s |  |
|  |  | native | 15.45 | 15.66 | 130 | 65.83 MiB/s | 6.35× |
|  |  | wasm | 44.97 | 63.04 | 41 | 22.62 MiB/s | 2.18× |
| micro/ascii-twin-1MB | the same text shape in ASCII only (control for the above) | TS v12 | 143 | 158 | 14 | 7.06 MiB/s |  |
|  |  | native | 14.47 | 14.66 | 139 | 69.56 MiB/s | 9.86× |
|  |  | wasm | 74.85 | 89.15 | 27 | 13.44 MiB/s | 1.90× |

## AutoComplete

| workload | what | engine | median | p95 | runs | throughput | × TS |
|---|---|---|---:|---:|---:|---:|---:|
| suggest/shell | suggestFor on 40 half-typed shell test inputs | TS v12 | 3.96 | 4.56 | 495 | 10,107 suggestions/s |  |
|  |  | native | 3.22 | 3.29 | 624 | 12,418 suggestions/s | 1.23× |
|  |  | wasm | 40.57 | 41.80 | 49 | 986 suggestions/s | 0.10× |
| suggest/jq | suggestFor on 40 half-typed jq test inputs | TS v12 | 3.93 | 4.56 | 497 | 10,180 suggestions/s |  |
|  |  | native | 8.19 | 8.48 | 243 | 4,884 suggestions/s | 0.48× |
|  |  | wasm | 64.84 | 65.60 | 31 | 617 suggestions/s | 0.06× |
| suggest/saya | suggestFor on 40 half-typed saya test inputs | TS v12 | 5.35 | 6.02 | 365 | 7,470 suggestions/s |  |
|  |  | native | 5.68 | 6.15 | 349 | 7,038 suggestions/s | 0.94× |
|  |  | wasm | 71.78 | 72.28 | 28 | 557 suggestions/s | 0.07× |
| suggest/calc | suggestFor on 14 half-typed calc test inputs | TS v12 | 0.417 | 0.448 | 1000 | 33,590 suggestions/s |  |
|  |  | native | 0.977 | 1.02 | 1000 | 14,329 suggestions/s | 0.43× |
|  |  | wasm | 6.78 | 7.00 | 294 | 2,064 suggestions/s | 0.06× |
| suggest/json | suggestFor on a small JSON document cut at 60% (360 units), ×20 | TS v12 | 0.908 | 1.43 | 1000 | 22,016 suggestions/s |  |
|  |  | native | 0.477 | 0.508 | 1000 | 41,943 suggestions/s | 1.91× |
|  |  | wasm | 21.75 | 22.21 | 92 | 920 suggestions/s | 0.04× |

## Where wasm's time goes

Parse workloads, wasm only, from a separate instrumented run of the same work (`split`): `wasm` is the call into wasm plus copying its flat `Uint32Array`s out (`takeExecResult`); `view` is the JS cursor and read-only tree view (`parseResult`); `Node tree` is materializing v12's `Node` objects from the view (`.ast`). Medians of per-iteration sums, so they add up to about the `full` median.

| workload | full | wasm | view | Node tree | Node tree share | native |
|---|---:|---:|---:|---:|---:|---:|
| parse/shell-1MB | 572 | 484 | 0.007 | 84.04 | 15% | 231 |
| parse/saya-1MB | 431 | 363 | 0.007 | 65.40 | 15% | 180 |
| parse/jq-256KB | 168 | 143 | 0.007 | 25.63 | 15% | 63.85 |
| tests/shell | 97.30 | 88.58 | 0.535 | 9.98 | 10% | 28.08 |
| tests/jq | 27.46 | 25.22 | 0.153 | 1.93 | 7% | 8.44 |
| tests/saya | 37.84 | 34.39 | 0.115 | 3.05 | 8% | 13.25 |
| tests/sed | 0.869 | 0.807 | 0.014 | 0.050 | 6% | 0.143 |
| tests/calc | 0.943 | 0.880 | 0.012 | 0.053 | 6% | 0.226 |
| tests/find | 0.445 | 0.401 | 0.007 | 0.039 | 9% | 0.067 |
| tests/glob | 0.295 | 0.254 | 0.004 | 0.038 | 13% | 0.046 |
| tests/date-expr | 0.549 | 0.504 | 0.009 | 0.040 | 7% | 0.078 |
| tests/regex-bre | 0.865 | 0.799 | 0.011 | 0.060 | 7% | 0.206 |
| tests/regex-ere | 0.556 | 0.508 | 0.006 | 0.043 | 8% | 0.133 |
| tests/regex-pcre | 0.904 | 0.824 | 0.006 | 0.075 | 8% | 0.308 |
| tests/typescript | 5.72 | 5.37 | 0.046 | 0.329 | 6% | 1.69 |
| micro/literal-choice-1MB | 71.34 | 26.99 | 0.006 | 44.29 | 62% | 11.21 |
| micro/regex-tokens-1MB | 116 | 58.84 | 0.006 | 56.99 | 49% | 25.11 |
| micro/pratt-flat-16KB | 16.98 | 10.40 | 0.004 | 6.51 | 38% | 4.78 |
| micro/pratt-nested-200 | 21.01 | 17.57 | 0.013 | 3.42 | 16% | 6.16 |
| micro/json-small | 35.70 | 21.60 | 0.075 | 13.75 | 39% | 6.66 |
| micro/json-1MB | 110 | 55.64 | 0.007 | 55.30 | 50% | 21.03 |
| micro/json-1MB-fail-at-end | 40.66 | 40.70 | 0.003 | 0.000 | 0% | 18.75 |
| micro/shell-1MB-fail-at-end | 460 | 462 | 0.008 | 0.001 | 0% | 232 |
| micro/shell-backticks-6 | 38.74 | 38.77 | 0.002 | 0.013 | 0% | 14.41 |
| micro/non-ascii-1MB | 44.97 | 29.61 | 0.006 | 18.80 | 39% | 15.45 |
| micro/ascii-twin-1MB | 74.85 | 35.15 | 0.006 | 39.33 | 53% | 14.47 |

## Memory

Peak RSS of each engine's whole timing process (`/usr/bin/time -l`, MiB): the runtime included (`noop` is a process that loads the engine and runs nothing), and every iteration, so it is an upper bound. Native's RSS overstates its heap: macOS's malloc keeps freed pages, so it grows with the iteration count while live bytes don't. The **native heap** columns are exact, from a counting allocator (`cpat-bench-peak`): the most live bytes while the grammar is built (and AutoComplete made), and the most live bytes above that during one iteration. **wasm linear** is the module's memory after the headline runs (it never shrinks, so it is its high-water mark); for builds that is after hundreds of builds nothing released (next section). wasm binary: 1907 KiB (700 KiB gzipped), pkg-node release.

| workload | TS v12 RSS | wasm RSS | native RSS | native heap: build | native heap: iteration | wasm linear |
|---|---:|---:|---:|---:|---:|---:|
| noop | 109.4 | 78.5 | 2.1 | 0.0 | 0.0 | 2.1 |
| build/shell | 353.7 | 586.7 | 9.9 | 3.5 | 3.5 | 476.9 |
| build/jq | 307.9 | 540.5 | 8.5 | 2.0 | 2.0 | 434.7 |
| build/saya | 311.3 | 635.2 | 9.0 | 2.4 | 2.4 | 526.8 |
| build/sed | 305.4 | 350.0 | 6.5 | 1.0 | 1.0 | 249.9 |
| build/calc | 234.6 | 477.7 | 6.1 | 0.8 | 0.8 | 369.4 |
| build/find | 303.5 | 266.1 | 5.8 | 0.7 | 0.7 | 158.1 |
| build/glob | 235.7 | 230.8 | 6.2 | 0.7 | 0.7 | 132.6 |
| build/date-expr | 257.5 | 467.5 | 6.0 | 1.0 | 1.0 | 366.5 |
| build/regex-bre | 290.2 | 349.6 | 6.0 | 0.8 | 0.8 | 250.4 |
| build/regex-ere | 292.4 | 347.0 | 5.7 | 0.8 | 0.8 | 246.9 |
| build/regex-pcre | 289.8 | 383.1 | 6.4 | 0.8 | 0.8 | 280.1 |
| build/typescript | 349.3 | 620.1 | 23.4 | 15.6 | 15.6 | 524.4 |
| build/synthetic-1200 | 4045.2 | 462.8 | 337.8 | 35.8 | 35.8 | 350.3 |
| parse/shell-1MB | 1406.9 | 1053.6 | 481.5 | 3.5 | 104.2 | 122.9 |
| parse/saya-1MB | 1184.7 | 900.6 | 436.7 | 2.4 | 77.7 | 110.6 |
| parse/jq-256KB | 871.6 | 576.7 | 474.2 | 2.0 | 64.3 | 62.8 |
| tests/shell | 532.6 | 356.3 | 50.7 | 3.5 | 34.9 | 25.5 |
| tests/jq | 669.3 | 368.2 | 81.1 | 2.0 | 62.6 | 41.4 |
| tests/saya | 609.5 | 370.7 | 60.9 | 2.4 | 45.0 | 31.6 |
| tests/sed | 168.8 | 331.3 | 5.9 | 1.0 | 0.1 | 2.6 |
| tests/calc | 266.6 | 317.5 | 5.8 | 0.8 | 0.3 | 2.2 |
| tests/find | 160.5 | 319.3 | 5.2 | 0.7 | 0.1 | 2.3 |
| tests/glob | 152.3 | 336.0 | 5.0 | 0.7 | 0.0 | 2.3 |
| tests/date-expr | 162.4 | 318.5 | 5.5 | 1.0 | 0.2 | 2.6 |
| tests/regex-bre | 160.8 | 327.7 | 5.2 | 0.8 | 0.1 | 2.3 |
| tests/regex-ere | 157.2 | 330.1 | 5.0 | 0.8 | 0.1 | 2.3 |
| tests/regex-pcre | 226.1 | 340.3 | 5.3 | 0.8 | 0.2 | 2.3 |
| tests/typescript | 322.5 | 382.8 | 131.1 | 15.6 | 92.9 | 71.1 |
| micro/literal-choice-1MB | 655.0 | 650.1 | 566.7 | 0.4 | 22.5 | 55.2 |
| micro/regex-tokens-1MB | 755.3 | 733.4 | 723.3 | 0.7 | 22.5 | 55.4 |
| micro/pratt-flat-16KB | 340.6 | 318.8 | 7.0 | 0.4 | 2.1 | 4.9 |
| micro/pratt-nested-200 | 326.3 | 337.3 | 6.7 | 0.4 | 1.4 | 2.8 |
| micro/json-small | 208.0 | 346.4 | 4.9 | 0.7 | 0.1 | 2.1 |
| micro/json-1MB | 1321.0 | 808.1 | 987.6 | 0.7 | 44.3 | 101.6 |
| micro/json-1MB-fail-at-end | 910.2 | 139.6 | 989.8 | 0.7 | 44.3 | 59.5 |
| micro/shell-1MB-fail-at-end | 2128.0 | 193.2 | 479.7 | 3.5 | 104.2 | 122.9 |
| micro/shell-backticks-6 | 280.4 | 122.5 | 38.7 | 3.5 | 26.7 | 23.2 |
| micro/non-ascii-1MB | 514.5 | 511.0 | 285.6 | 0.8 | 11.5 | 34.6 |
| micro/ascii-twin-1MB | 642.5 | 663.1 | 492.1 | 0.8 | 22.6 | 55.5 |
| suggest/shell | 268.0 | 107.3 | 11.5 | 3.5 | 1.6 | 8.6 |
| suggest/jq | 282.1 | 111.6 | 16.7 | 2.0 | 7.5 | 16.9 |
| suggest/saya | 273.1 | 119.9 | 22.8 | 2.4 | 11.5 | 23.8 |
| suggest/calc | 261.6 | 101.3 | 6.0 | 0.8 | 0.2 | 2.8 |
| suggest/json | 212.0 | 98.6 | 5.3 | 0.7 | 0.2 | 3.5 |

## Builds that release what they built

Build workloads run back to back like every other workload (the headline). wasm-bindgen frees a pattern's wasm memory from a `FinalizationRegistry`, which runs only after a garbage collection and only when the event loop turns, so a synchronous loop of `Grammar.parseString` keeps every grammar it built: wasm memory grows and later builds get slower. `released` times the same builds with a full GC and two event-loop turns between iterations (untimed), for both JS engines; it ran first, in a fresh process. The forced GC leaves caches cold, so small builds look slower there for both engines.

| workload | TS v12 | TS v12 released | wasm | wasm released | wasm linear: released | wasm linear: back to back |
|---|---:|---:|---:|---:|---:|---:|
| build/shell | 13.29 | 19.25 | 8.86 | 3.75 | 5.6 | 476.9 |
| build/jq | 5.50 | 10.64 | 5.69 | 2.12 | 3.9 | 434.7 |
| build/saya | 4.61 | 9.09 | 6.11 | 2.24 | 4.4 | 526.8 |
| build/sed | 0.663 | 2.37 | 0.722 | 1.01 | 2.8 | 249.9 |
| build/calc | 1.12 | 3.44 | 1.89 | 0.953 | 2.4 | 369.4 |
| build/find | 0.777 | 2.64 | 0.446 | 0.772 | 2.3 | 158.1 |
| build/glob | 0.302 | 1.34 | 0.286 | 0.566 | 2.3 | 132.6 |
| build/date-expr | 0.889 | 2.92 | 1.08 | 1.38 | 2.8 | 366.5 |
| build/regex-bre | 0.498 | 1.87 | 0.510 | 0.836 | 2.6 | 250.4 |
| build/regex-ere | 0.350 | 1.52 | 0.437 | 0.749 | 2.3 | 246.9 |
| build/regex-pcre | 0.599 | 2.14 | 0.593 | 0.880 | 2.6 | 280.1 |
| build/typescript | 12.19 | 19.19 | 62.88 | 4.25 | 21.3 | 524.4 |
| build/synthetic-1200 | 3436 | 3424 | 137 | 41.72 | 27.5 | 350.3 |

## Machine

```
Darwin Mac.ht.home 25.4.0 Darwin Kernel Version 25.4.0: Thu Mar 19 19:33:50 PDT 2026; root:xnu-12377.101.15~1/RELEASE_ARM64_T6050 arm64
Apple M5 Pro
64 GiB RAM, 18 cores (6 performance, 12 efficiency)
node v24.16.0
rustc 1.99.0 (b940084d7 2026-09-28)
wasm-pack 0.15.0
```

