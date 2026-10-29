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
