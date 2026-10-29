# Findex Engineering Benchmarks

Reproducible measurements. Every benchmark entry records: ID, source commit,
binary identity, dataset, environment, hardware, configuration, command,
repetition count, baseline result, new result, absolute/relative difference,
accuracy impact, resource impact, validity, promotion decision, and evidence
location.

Historical results are never replaced; both baseline and later measurements are
preserved.

## Benchmark conventions

- All timestamps UTC.
- `NOT MEASURED` means no data; never an estimated value.
- Benchmarks are valid only when hardware and dataset match between baseline
  and new result.
- Raw structured evidence lives under `validation/results/<run-id>/`.

## Benchmark index

| ID | Source commit | Dataset | Platform | Status | Baseline | Current | Change | Evidence |
|---|---|---|---|---|---|---|---|---|
| FX-BM-001 | 501bbae | 65 historical issues | Windows x64 | Historical | hit@1 40.0% | hit@1 ~44.6% | +4.6pp | validation/results/* |
| FX-BM-002 | 501bbae | 65 historical issues | Windows x64 | Historical | recall@10 79.2% | recall@10 ~80.8% | +1.6pp | validation/results/* |
| FX-BM-003 | 501bbae | 115 navigation queries | Windows x64 | Historical | hit@1 0.982 (Findex compact) | same | 0 | validation/results/20261027-performance-comparison/ |
| FX-BM-004 | 501bbae | 115 navigation queries | Windows x64 | Historical | tokens 222 (Findex compact) | same | 0 | validation/results/20261027-performance-comparison/ |
| FX-BM-005 | 0490018 | — | Windows x64 | NOT MEASURED | — | — | — | None |
| FX-BM-008 | 91d7ab2 (code = main f0501da) | 65 historical issue titles, frozen | Windows x64 | Measured 2026-10-30 | hit@1 0.446, MRR 0.581, hit@10 0.831, gold recall@10 0.808 (`findex context`) | git grep words: 0.062 / 0.253 / 0.646 / 0.626; `findex search` (frozen main caf33fa): 0.154 / 0.297 / 0.692 / 0.667 | vs FX-BM-001/002 +0.0pp: confirms 44.6% / 80.8% on HEAD | validation/results/20261030-localization-head/ |
| FX-BM-009 | HEAD + BM25 score rounding | 115 frozen definition queries (55 held-out), three native MCP servers | Windows x64 | Measured 2026-10-30 | 2026-10-27 run (`20261027-final-comparison`): Findex compact 222 (55) / 263.8 (115) mean raw tokens, hit@1 0.982 / 0.974; codebase tree 240.3 / 266.4; Graphify 2,663 / 2,275 | Findex compact 206 (55) / 243.0 (115) tokens, hit@1 0.982 / 0.974, p50 2.5 ms; competitors unchanged | -7.2% / -7.9% Findex tokens, accuracy and hit lists unchanged; codebase-memory still lower on `requests` (207.4 vs 209.4) | validation/results/20261030-payload-head/ (aborted first attempt kept in `-aborted`) |
| FX-BM-010 | HEAD, repaired live harness | 4 single-definition tasks (fmt x2, requests, ripgrep), Claude Haiku via subscription, 1 repetition | Windows x64 | Measured 2026-10-30 (R4, R5) | R4: Findex 4/4, 78,951 tokens/session; Graphify 4/4, 80,991; codebase-memory 3/4, 96,272 | R5 (Findex hint names the upfront `findex_context`): Findex 4/4, 54,853; Graphify 4/4, 74,736; codebase-memory 3/4, 89,419 | R5: Findex -26.6% vs Graphify, -38.7% vs codebase-memory; unchanged arms moved about 7% between rounds; four tasks, one repetition | validation/results/20261030-claude-live-r4/, validation/results/20261030-claude-live-r5/ |

## Notes

- FX-BM-001/002 are historical reported measurements from commit 501bbae; they
  have not been re-verified on HEAD (0490018). See ISSUE FX-20261029-001.
- FX-BM-005/006/007 are placeholders for HEAD benchmarks; no profiling or
  timing data has been collected on the current commit.

## Planned benchmarks

- Cold indexing time (small/medium/large/very large repos)
- Incremental indexing time after single-file and bulk edits
- Query p50/p95/p99 latency
- Memory peak during indexing and query
- Candidate recall@k independent of final ranking
- FTS5 BM25 score ordering correctness (fixture-based)

- FX-BM-008 (2026-10-30): the historical hit@1 44.6% and recall@10 80.8% are re-verified on HEAD with `validation/harness/localization.py` (no model calls). `findex context` returns a mean of 12,950 bytes per query; the 60% / 90% targets are still unmet. The run used `FINDEX_VALIDATION_HOME` outside `%LOCALAPPDATA%` because the Claude desktop app virtualizes writes below it.
