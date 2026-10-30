# Report-routing candidate vs previous binary, exposed v1 (65 tasks) — 2026-10-30

Same two binaries as `../20261030-localization-v2-hybrid`. v1 has been examined in many earlier rounds; it
is a regression check, not a fresh holdout. Budget is the harness default of 8,000 tokens, not the 1,200 of
earlier rounds (the previous binary's hit@1 0.5846 and recall@10 0.8692 are identical at both budgets; MRR is
0.6958 here against 0.6962 at 1,200), so this run is not plotted on the 1,200-token progress lane.

| all 65 | previous | candidate |
|---|---:|---:|
| hit@1 | 0.585 | 0.585 |
| hit@3 | 0.769 | 0.800 |
| hit@10 | 0.892 | 0.923 |
| gold recall@10 | 0.869 | 0.900 |
| MRR | 0.696 | 0.706 |

Changed first-gold ranks: ripgrep-2479 and zod-3437 (absent before, rank 3 now). The v1 test half
(28) is unchanged (hit@1 0.607, recall@10 0.893). Recall@10 reaches 0.900 here; that figure is on tasks the
design was informed by and does not transfer: the fresh set gives 0.761.
