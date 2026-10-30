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
| FX-BM-011 | HEAD (no Findex change since R5), four fresh tasks | 4 new single-definition tasks (ripgrep, fmt, requests x2), Claude Haiku via subscription, 1 repetition | Windows x64 | Measured 2026-10-30 (R6) | R5 tasks: Findex 4/4, 54,853 tokens/session | R6: Findex 4/4, 54,400; Graphify 3/4, 103,064; codebase-memory 2/4, 109,448 (invalid sessions counted as failures) | Pooled R5+R6 (8 tasks): Findex 8/8, Graphify 7/8, codebase-memory 5/8; advantage is mainly the upfront tool, not retrieval quality | validation/results/20261030-claude-live-r6/ |
| FX-BM-012 | HEAD + vendored/docs path weights in concept ranking | 65 frozen issue-localization tasks (37 dev / 28 test), no model calls | Windows x64 | Measured 2026-10-30 (R7) | R3: hit@1 0.446, hit@10 0.831, MRR 0.581, recall@10 0.808 | hit@1 0.462, hit@10 0.846, MRR 0.593, recall@10 0.823 | Five tasks up (requests-711 rank 12 to 3, requests-971 9 to 7, requests-575 2 to 1, fmt-3631 12 to 11, fmt-2642 6 to 5), none down; weights 0.5 / 0.7 fixed a priori, not fitted; targets 0.60 / 0.90 unmet; rivals not measurable on this dataset | validation/results/20261030-localization-weights/ |
| FX-BM-013 | HEAD + BM25 length-normalisation relaxed (b 0.75 to 0.3) in concept ranking | 65 frozen issue-localization tasks (37 dev / 28 test), no model calls | Windows x64 | Measured 2026-10-30 (R8) | R7: hit@1 0.462, hit@10 0.846, MRR 0.593, recall@10 0.823 (dev 0.459 / test 0.464 hit@1) | hit@1 0.538, hit@10 0.892, MRR 0.670, recall@10 0.869 (dev 0.541 / test 0.536 hit@1) | 19 tasks up, 5 down: ripgrep-2524 1 to 3, ripgrep-2574 1 to 2, ripgrep-2645 4 to 6, ripgrep-2658 11 to 13, fmt-4053 11 to 12; b = 0.3 is a round value picked before measuring (no literature value found for source files), no second value tried; targets 0.60 / 0.90 still unmet | validation/results/20261030-localization-length/ |
| FX-BM-014 | HEAD + symbol-name field in concept ranking; rivals measured offline | 65 frozen issue-localization tasks (37 dev / 28 test), Findex vs Graphify (`extract --code-only`, `query --budget 8000`) vs codebase-memory (`index_repository`, `search_graph`), no model calls | Windows x64 | Measured 2026-10-30 (R9) | R8 Findex: hit@1 0.538, MRR 0.670, recall@10 0.869; rivals not measured | Findex hit@1 0.585 (dev 0.568 / test 0.607), hit@10 0.892, MRR 0.697, recall@10 0.869; codebase-memory 0.246 / 0.723 / 0.400 / 0.705; Graphify 0.123 / 0.785 / 0.322 / 0.723; git grep 0.062 / 0.646 / 0.253 / 0.626 | 11 tasks up, 2 down vs R8 (ripgrep-1757 1 to 2, fmt-1747 5 to 6); symbol-name weight 1 chosen on dev from five configs (proximity feature tried and dropped), then confirmed once on test; rivals use their documented query surfaces with the raw issue title and were not tuned for natural-language input; Findex returns more bytes per query (13.0 KB mean) than codebase-memory (4.8 KB); recall@10 target 0.90 still unmet | validation/results/20261030-localization-symbols/ (`localization.json`, `localization-rivals.json`) |
| FX-BM-015 | HEAD (symbol-name field build), live harness `localization_live.py` | 8 held-out test-split issues (fmt, requests, ripgrep, zod; 2 each), Claude Haiku via subscription, 20-turn cap, 1 repetition, 32 sessions | Windows x64 | Measured 2026-10-30 (R10) | No earlier live localization measurement | Correct first file: Findex 4/8, codebase-memory 4/8, Graphify 3/8, no tool 3/8; valid sessions 5/6/4/4 of 8; tokens per session 420,352 / 716,021 / 657,830 / 631,956 | Findex ties codebase-memory on accuracy at 33-41% fewer tokens; eight issues, one repetition, differences within noise; three Findex turn-cap failures despite offline gold rank 1 on seven of eight issues; aborted 10-turn attempt kept separately (37 sessions used in total) | validation/results/20261030-localization-live/ (`NOTE.md`) |
| FX-BM-016 | HEAD + `ranked files` line in concept output | Same 8 issues, Findex arm only, 8 sessions | Windows x64 | Measured 2026-10-30 (R11) | R10 Findex: 4/8 correct, 5/8 valid, 13.1 turns, 420,352 tokens per session | 6/8 correct, 7/8 valid, 5.0 turns, 143,014 tokens per session | 2 issues up (requests-3844, zod-1046), none down; -66% tokens per session; designed after reading the R10 failures on these issues, so not independent; rivals not re-run; remaining losses ripgrep-2122 (model overrode the ranking) and zod-2184 (gold ranks 16th) | validation/results/20261030-localization-live-ranked/ (`NOTE.md`) |
| FX-BM-017 | HEAD (ranked-files build), Codex live harness `tool_comparison_live.py --client codex` | 4 fresh single-definition tasks (ripgrep x2, fmt, requests), `gpt-5.6-luna` at low effort via ChatGPT subscription, 1 repetition, 12 sessions | Windows x64 | Measured 2026-10-30 (R12) | R0 Codex navigation (2 tasks): Findex 1.00 at 74,764 tokens per session, Graphify 1.00 at 108,640 | Findex 4/4 correct, 4/4 valid, 62,825 tokens per session; Graphify 3/4, 4/4, 165,062; codebase-memory 3/4, 3/4, 181,407 | Findex -62% total tokens vs Graphify, -65% vs codebase-memory; on uncached input -10% and -23%; four tasks, one repetition, different task sets per round | validation/results/20261030-codex-live-r12/ (`NOTE.md`) |

## Notes

- **FX-BM-018, 2026-10-30 retrieval-integrity continuation (base 28842c5,
  branch fix/retrieval-integrity):** paired 65-task localization at 1,200 tokens retains
  58.46% hit@1 / 86.92% gold recall@10. Explicit JSON rank propagation moves
  fmt-2695 gold rank 3 to 4 (MRR 0.696150 to 0.694868); no ranking weights
  changed. 115 native MCP navigation hit lists remain identical. Real
  rust-analyzer SCIP: 6/6 CLI/MCP cases. Native stale-prefix semantic oracle:
  frozen binary fails, candidate passes. Windows source gates: 249 Rust / 63
  Python tests, fmt, Clippy and release pass. Baseline is the session-start
  executable, not an independently rebuilt source baseline. See
  [binary identities, measurements and limits](RETRIEVAL_INTEGRITY_20261030.md)
  and [raw evidence](../../validation/results/20261030-retrieval-integrity).

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
