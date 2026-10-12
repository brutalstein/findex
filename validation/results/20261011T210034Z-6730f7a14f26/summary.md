# Validation run 20261011T210034Z-6730f7a14f26

- commit `6730f7a14f26d4ac9fe23595b1698d409a1c5fdb` on `validation/production-engineering` (dirty: True)
- Windows-11-10.0.26200-SP0, AMD Ryzen 9 8945HX with Radeon Graphics, 32 logical CPUs, 31.2 GiB RAM
- rustc: rustc 1.97.0 (2d8144b78 2026-07-07)
- Claude Code: 2.1.281 (Claude Code)

## Components

- tests: EXECUTED
- retrieval: EXECUTED
- relations: EXECUTED
- performance: EXECUTED
- memory: EXECUTED
- cpu: EXECUTED
- tokens: EXECUTED
- comparisons: EXECUTED
- security: EXECUTED
- compatibility: EXECUTED

## Static checks and tests

- fmt: PASS
- clippy: PASS
- test: PASS (86 passed, 0 failed, 0 ignored)
- release_build: PASS
- audit: PASS

## Retrieval (all queries)

| system | R@1 | R@10 | MRR | spanR@1 | top1 FP | payload B | p95 ms |
|---|---|---|---|---|---|---|---|
| gitgrep | 0.790 | 0.980 | 0.860 | 0.597 | 0.210 | 175781 | 140.0 |
| findex-baseline | 0.695 | 0.964 | 0.790 | 0.522 | 0.305 | 1245 | 55.3 |
| findex-current | 0.984 | 1.000 | 0.991 | 0.972 | 0.016 | 1390 | 30.7 |
| findex-current-symbol | 0.971 | 0.979 | 0.974 | 0.964 | 0.007 | 442 | 4.2 |

## Claude Code A/B (all tasks)

- arm A: success None, total tokens mean None, est. cost mean None, n=0
- arm B: success None, total tokens mean None, est. cost mean None, n=0

## Regressions (current vs baseline)

- retrieval/all/payload_bytes_mean: 1244.7834890965732 -> 1389.556074766355 (11.63%)
- retrieval/identifier_words/payload_bytes_mean: 1750.5355648535565 -> 1960.702928870293 (12.01%)
- retrieval/identifier_words/dev/payload_bytes_mean: 1755.1538461538462 -> 1964.4023668639054 (11.92%)
- retrieval/identifier_words/heldout/payload_bytes_mean: 1739.3857142857144 -> 1951.7714285714285 (12.21%)
- retrieval/line_terms/payload_bytes_mean: 1739.7154471544716 -> 1977.219512195122 (13.65%)
- retrieval/line_terms/dev/payload_bytes_mean: 1737.088888888889 -> 1972.1444444444444 (13.53%)
- retrieval/line_terms/heldout/payload_bytes_mean: 1746.878787878788 -> 1991.060606060606 (13.98%)
- retrieval/split/dev/payload_bytes_mean: 1248.8429203539822 -> 1392.08407079646 (11.47%)
- retrieval/split/heldout/payload_bytes_mean: 1235.1263157894737 -> 1383.542105263158 (12.02%)
- performance/tokio/db_bytes: 8937472 -> 10059776 (12.56%)
- performance/vscode/cold_index_peak_working_set_bytes: 26165248 -> 33996800 (29.93%)
- performance/vscode/db_bytes: 123953152 -> 137555968 (10.97%)
- performance/vscode/concurrency_rps: 93.09831904032752 -> 53.676937301301464 (-42.34%)

## Not passing

- FAIL: 0
- BLOCKED: 41
- SKIPPED: 0
- NOT_RUN: 0

Details: failures.json, regressions.json. Checksums: checksums.sha256.
