# Report-routing candidate vs previous binary, fresh holdout (v2) — 2026-10-30

Paired run on the same 55 frozen tasks: `findex-baseline-context` = release `6528a1423f14`, `findex-context` =
release `b767d40c2b` (commit d4718e9: a task with an identifier plus at least two descriptive words and no
explicit "where ... defined" also runs the concept search; the identifier evidence stays first).

| all 55 | previous | candidate |
|---|---:|---:|
| hit@1 | 0.364 | 0.345 |
| hit@3 | 0.691 | 0.727 |
| hit@10 | 0.745 | 0.800 |
| gold recall@10 | 0.706 | 0.761 |
| MRR | 0.529 | 0.538 |

Dev half (28, per-task rows read): hit@1 0.250 -> 0.214, hit@10 0.571 -> 0.679. Test half (27, aggregate
only): identical, hit@1 0.481, hit@10 0.926 — no test task is routed differently, so this half neither
confirms nor refutes the change. The evidence for it is the dev half and the exposed v1 run
(`../20261030-localization-v1-hybrid`). One hit@1 is lost (ripgrep-2243, rank 1 -> 2).

The 60% hit@1 and 90% gold-recall@10 targets are not met on this fresh set.
