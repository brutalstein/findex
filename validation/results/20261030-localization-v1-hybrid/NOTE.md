# Report-routing candidate vs previous binary, exposed v1 (65 tasks) — 2026-10-30

Same two binaries as `../20261030-localization-v2-hybrid`. v1 has been examined in many earlier rounds; it
is a regression check, not a fresh holdout.

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
