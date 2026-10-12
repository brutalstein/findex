# Validation run 20261012T005742Z-746f47e6bb69

- commit `746f47e6bb69c31878814d255e1a0aeffaee3fe7` on `validation/production-engineering` (dirty: False)
- Windows-11-10.0.26200-SP0, AMD Ryzen 9 8945HX with Radeon Graphics, 32 logical CPUs, 31.2 GiB RAM
- rustc: rustc 1.96.1 (31fca3adb 2026-06-26)
- Claude Code: 2.1.281 (Claude Code)

## Components

- tests: EXECUTED
- retrieval: EXECUTED
- relations: EXECUTED
- performance: EXECUTED
- memory: EXECUTED
- cpu: EXECUTED
- tokens: NOT_RUN
- comparisons: NOT_RUN
- security: EXECUTED
- compatibility: EXECUTED

## Static checks and tests

- fmt: PASS
- clippy: FAIL
- test: PASS (89 passed, 0 failed, 0 ignored)
- release_build: PASS
- audit: PASS

## Retrieval (all queries)

| system | R@1 | R@10 | MRR | spanR@1 | top1 FP | payload B | p95 ms |
|---|---|---|---|---|---|---|---|
| gitgrep | 0.790 | 0.980 | 0.860 | 0.597 | 0.210 | 175781 | 110.9 |
| findex-baseline | 0.695 | 0.964 | 0.790 | 0.522 | 0.305 | 1245 | 41.2 |
| findex-current | 0.984 | 1.000 | 0.991 | 0.970 | 0.016 | 1392 | 21.9 |
| findex-current-symbol | 0.971 | 0.979 | 0.974 | 0.964 | 0.007 | 442 | 3.0 |

## Regressions (current vs baseline)

- retrieval/all/payload_bytes_mean: 1244.7834890965732 -> 1391.8021806853583 (11.81%)
- retrieval/identifier_words/payload_bytes_mean: 1750.5355648535565 -> 1960.702928870293 (12.01%)
- retrieval/identifier_words/dev/payload_bytes_mean: 1755.1538461538462 -> 1964.4023668639054 (11.92%)
- retrieval/identifier_words/heldout/payload_bytes_mean: 1739.3857142857144 -> 1951.7714285714285 (12.21%)
- retrieval/line_terms/payload_bytes_mean: 1739.7154471544716 -> 1977.219512195122 (13.65%)
- retrieval/line_terms/dev/payload_bytes_mean: 1737.088888888889 -> 1972.1444444444444 (13.53%)
- retrieval/line_terms/heldout/payload_bytes_mean: 1746.878787878788 -> 1991.060606060606 (13.98%)
- retrieval/split/dev/payload_bytes_mean: 1248.8429203539822 -> 1393.5796460176991 (11.59%)
- retrieval/split/heldout/payload_bytes_mean: 1235.1263157894737 -> 1387.5736842105264 (12.34%)
- retrieval/symbol_exact/dev/latency_p95_ms: 6.8111999999018735 -> 7.883100000071863 (15.74%)
- retrieval/symbol_exact/heldout/payload_bytes_mean: 635.2873563218391 -> 704.7126436781609 (10.93%)
- performance/tokio/db_bytes: 8937472 -> 10059776 (12.56%)
- performance/vscode/cold_index_peak_working_set_bytes: 25907200 -> 32374784 (24.96%)
- performance/vscode/db_bytes: 123953152 -> 137564160 (10.98%)

## Not passing

- FAIL: 1
- BLOCKED: 0
- SKIPPED: 0
- NOT_RUN: 2

Details: failures.json, regressions.json. Checksums: checksums.sha256.
