# Experiment matrix

| # | Experiment | Systems / arms | Data | Status | Evidence |
|---|---|---|---|---|---|
| E1 | Issue localization (held-out) | gitgrep-words, findex-main, findex-context | 65 real fixed issues, 4 corpora, split dev 37 / test 28 | **EXECUTED** | `results/20261012T101245Z-3a2e7de81241/localization*.json` |
| E2 | Call sites vs rust-analyzer SCIP | findex-context | ripgrep (dev), tokio (held-out), 80 functions each | **EXECUTED** | `…3a2e7de81241/callsites-*/` |
| E3 | Cross-language edge precision (Python/TS/C++ SCIP) | — | — | **BLOCKED**: scip-python, scip-typescript, scip-clang not installed | — |
| E4 | Claude Code A/B/C/D/E, 8 tasks × 5 arms × 3 reps | A–E | ripgrep, tokio, requests, findex | **EXECUTED** 120/120 (Run 3); earlier runs PARTIAL/BLOCKED | `results/20261012T105254Z-397bf78730ba` (`ab_stats.json`) |
| E5 | Negative controls n1/n2 (no Findex needed) | A–E | 30 sessions | **EXECUTED** (3 reps; Findex never called) | `…105254Z-397bf78730ba` |
| E6 | Black-box security + `cargo audit` | release build `132b29d` | 30 checks | **EXECUTED** | `results/20261012T102934Z-599cd1e65674/` |
| E7 | Cold-index peak-memory ablation (REMAINING_LIMITATIONS #17) | baseline 44f1a38, main caf33fa, head, head−terms, head−symbols | vscode, 3 cold reps | **EXECUTED** | see PERFORMANCE_RESULTS |
| E8 | Concurrent relations-DB first open | unit test, 20 × 8 threads | — | **EXECUTED** (reproduced, fixed) | `concurrent_first_opens_of_a_new_database_all_succeed` |
| E9 | Symbol retrieval regression (642 queries) | — | — | **NOT RE-RUN** (search ranking unchanged) | last: `20261012T001625Z-bf4967d90524` |
| E10 | OneDrive / network volumes, mass branch switch | — | — | **NOT RUN** | REMAINING_LIMITATIONS #8, #9 |

Rules applied: splits fixed before evaluation; held-out results inspected
only in aggregate; no threshold tuned on held-out data; skipped and blocked
items are never counted as passes.
