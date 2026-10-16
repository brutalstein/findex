# Experiment matrix

| # | Experiment | Systems / arms | Data | Status | Evidence |
|---|---|---|---|---|---|
| E1 | Issue localization (held-out) | gitgrep-words, findex-main, findex-context | 65 real fixed issues, 4 corpora, split dev 37 / test 28 | **EXECUTED** | `results/20261012T101245Z-3a2e7de81241/localization*.json` |
| E2 | Call sites vs rust-analyzer SCIP | findex-context | ripgrep (dev), tokio (held-out), 80 functions each | **EXECUTED** | `…3a2e7de81241/callsites-*/` |
| E3 | Cross-language edge precision (Python/TS/C++ SCIP) | — | — | **BLOCKED**: scip-python, scip-typescript, scip-clang not installed | — |
| E4 | Claude Code A/B/C/D/E, 8 tasks × 5 arms × 3 reps | A–E | ripgrep, tokio, requests, findex | **EXECUTED** 120/120 (Run 3, negative); earlier runs PARTIAL/BLOCKED | `results/20261012T105254Z-397bf78730ba` (`ab_stats.json`) |
| E4b | Claude Code Run 4: 12 tasks (4 held-out) × arms A, D, F × 3 reps | A, D, F | + fmt, zod, pybind11 | **EXECUTED** 108/108 (positive on navigation) | `results/20261016T105620Z-ac4ea21dd692` (`ab_stats.json`) |
| E5 | Negative controls n1/n2 (no Findex needed) | A–E | 30 sessions | **EXECUTED** (3 reps; Findex never called) | `…105254Z-397bf78730ba` |
| E6 | Black-box security + `cargo audit` | release build `132b29d` | 30 checks | **EXECUTED** | `results/20261012T102934Z-599cd1e65674/` |
| E7 | Cold-index peak-memory ablation (REMAINING_LIMITATIONS #17) | baseline 44f1a38, main caf33fa, head, head−terms, head−symbols | vscode, 3 cold reps | **EXECUTED** | see PERFORMANCE_RESULTS |
| E8 | Concurrent relations-DB first open | unit test, 20 × 8 threads | — | **EXECUTED** (reproduced, fixed) | `concurrent_first_opens_of_a_new_database_all_succeed` |
| E9 | Symbol retrieval regression (642 queries) + security re-run | findex-current `fb1b000` | 7 corpora | **EXECUTED** (unchanged: R@1 0.984, MRR 0.991; security 30/30) | `results/20261016T105926Z-fb1b000c2bef` |
| E10 | Mass branch switch under watcher; local OneDrive folder | findex-current `384393c` | ripgrep 14.1.1 ↔ 13.0.0; this repository | **EXECUTED** (both pass); SMB, cloud-only placeholders NOT RUN | `results/20261016T105742Z-384393cfdee2` |
| E11 | Coverage claim vs `git grep -w` | findex-context `ac4ea21` | 279 golden names, 7 corpora | **EXECUTED** 279/279 | `results/20261016T105518Z-ac4ea21dd692/coverage/` |

Rules applied: splits fixed before evaluation; held-out results inspected
only in aggregate; no threshold tuned on held-out data; skipped and blocked
items are never counted as passes.
