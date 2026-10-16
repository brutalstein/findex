# Performance results

Release builds, this machine (see BASELINE), `validation/harness/perf.py
--systems current --corpora ripgrep,tokio --reps 3`, same session, run one
after the other: `results/20261016T172418Z-edd41bf1f3a6/perf-*`.
`fbc73ad` = before secure delete; `edd41bf` = with `secure_delete` and FTS5
`secure-delete`.

| Corpus | Metric | `fbc73ad` | `edd41bf` |
|---|---|---|---|
| ripgrep (195 files) | cold index, 3 reps (s) | 0.467 / 0.440 / 0.441 | 0.604 / 0.439 / 0.463 |
| | no-op reconcile (s) | 0.10 | 0.09 |
| | DB size | 5,029,888 B | 5,029,888 B |
| | search p50 / p95 (ms) | 10.5 / 22.6 | 11.0 / 24.2 |
| | watcher edit / create / rename / delete | 25/25, 10/10, 10/10, 10/10 correct | same |
| tokio (763 files) | cold index, 3 reps (s) | 0.995 / 0.949 / 0.951 | 1.033 / 0.993 / 0.975 |
| | DB size | 10,059,776 B | 10,059,776 B |
| | search p50 / p95 (ms) | 13.6 / 27.8 | 12.8 / 25.4 |
| | 4-client concurrency | 217 rps, 0 errors | 218 rps, 0 errors |

Reading: cold-index medians differ by +5 % (ripgrep) and +4 % (tokio); with
3 repetitions and one 0.60 s outlier this is not distinguishable from noise.
Database size is identical. Query latency and watcher correctness are
unchanged. Status: PASS (no measurable regression); the size of a real
secure-delete cost on large deletions was not isolated (UNVERIFIED).

## Context budget (A8)

`findex context` on 40 ripgrep golden names × 2 tasks (callers, definition),
rendered text length / 4 vs. requested budget, before the fix (`01ff624`):

| Budget | p50 | p95 | max | reports over budget |
|---|---|---|---|---|
| 200 | 0.81 | 1.29 | 1.32 | 24/80 |
| 500 | 0.40 | 1.06 | 1.12 | 8/80 |
| 2000 | 0.10 | 0.52 | 1.00 | 1/80 |

`d60e34b` counts the whole rendered report; the unit test
`rendered_report_stays_within_budget` asserts it at 200/400/800 tokens.
The corpus re-measurement after the fix is in CONTEXT_ENGINE_RESULTS.

Not measured in this campaign: vscode-scale corpus, memory under long MCP
sessions, Windows 10.
