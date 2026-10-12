# Frozen baseline

- Spec baseline commit: `493627f`. vNext branch base: `caf33fa` (= `origin/main`
  when vNext started; adds only validation records on top of `493627f`).
- Frozen baseline binary: `%FINDEX_VALIDATION_HOME%\bin\main-caf33fa\findex.exe`
  (release build of `caf33fa`, used as `findex-main` and as arms B/C).
- Source of baseline numbers: `validation/FINAL_VALIDATION_REPORT.md` and the
  run directories it cites. CI run 38191592325 (green; build-provenance
  attestation verified).

## Baseline measurements (from the production-validation campaign)

| Area | Value | Run |
|---|---|---|
| Symbol retrieval, 642 queries, file R@1 / MRR | 0.984 / 0.991 (held-out R@1 0.974) | `20261012T001625Z-bf4967d90524` |
| `git grep` baseline R@1 | 0.790 | same |
| vscode cold index (6,877 files) | 15.5 s | `20261012T005742Z-746f47e6bb69` |
| vscode cold-index peak working set | 30.9 MiB (start build 24.7 MiB, +25 %) | same |
| Black-box security | 30/30 | `bf4967d` run |
| Claude A/B (Haiku, 36 sessions) | B vs A total tokens +5.5 %, paired +10.2k ± 36.8k (n.s.); Findex used in 3/18 sessions | `20261012T001625Z-bf4967d90524` |

## Baseline defects carried into vNext

1. Findex tools are deferred in Claude Code: each first use costs a
   `ToolSearch` turn (~26.5k tokens of re-read context).
2. No tool answers a task ("who calls X, which tests cover it") in one call.
3. Golden labels written by the same agent that tuned ranking.
4. A/B sessions contaminated by the operator's `~/.claude/CLAUDE.md` and skills
   (found in vNext; see CLAUDE_AB_RESULTS).
5. REMAINING_LIMITATIONS #6 (relations DB open without BUSY retry) and #17
   (unexplained +25 % cold-index peak memory).
