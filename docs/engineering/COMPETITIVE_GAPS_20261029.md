# Competitive gaps and measured changes — 2026-10-29

This is a prioritized engineering assessment, not a claim of universal victory.
The user requested a plan, implementation, then renewed subscription testing.
Implementation follows [the recorded plan](../superpowers/plans/2026-10-28-context-locality.md).

| Gap | Evidence before this iteration | How to improve and acceptance gate |
|---|---|---|
| Issue localization | 40.0% hit@1; 79.23% gold recall@10 on 65 public issues | Weight exact filename words and choose a five-line window covering distinct query terms. Compare against a frozen binary at 1,200 tokens; require no aggregate dev regression, then freeze before test aggregate. Longer-term targets remain 60%/90%. |
| Live completion reliability | 1/2 primary passes; one unexplained 300-second timeout; successful diagnostic excluded from primary | Retain partial streams, pending MCP call IDs, completed/error counts, last observed phase and final-usage availability. Same timeout and no automatic retry. A successful new run cannot explain or erase the old timeout. |
| Preparation cost | Findex 42.09 seconds total; Rust SCIP contributes about 40.7 seconds, but the diagnostic uses context only | Make compiler snapshot setup explicit and opt-in. Record the mode in the frozen manifest; report preparation separately. This trades away imported semantics for ordinary lexical/syntax task navigation, not free equivalent semantics. |
| Semantics and call precision | Six real compiler fixtures pass; ordinary context is lexical/syntactic. Prior Tokio conditional precision 88.1%, with abstention reducing coverage | Retain explicit compiler identity lookup and uncertainty. Next: evaluate a compiler-backed context adapter outside the index crate to avoid a dependency cycle, with precision AND recall/coverage on independently labelled cases. This adapter is not implemented or claimed here. |
| Structural summary token size | codebase-memory tree output is smaller on three earlier corpora | Keep lossless compact search/symbol output and unchanged navigation controls. Next: compare equal-content summaries including scope and missing-coverage facts, not a shorter response that silently drops evidence. No new tree compression win is claimed here. |
| Follow-up work during bugfixes | New pilot: Findex 16 native commands, alternatives 10 each; model wall time and total tokens lose to codebase-memory | Next: evaluate bounded enclosing-function bodies in the existing context response so source-reading follow-ups can decrease while verification remains. Gate on both end-to-end tokens/time and the existing localization/budget controls. Counts mix reads, edits and verification, so they do not prove every extra command is unnecessary. No product code is changed after freezing the live candidate. |
| All languages and project sizes | Nine syntax languages; explicit imported snapshots; 100k synthetic-file tests on earlier executables | Keep ingestion/query caps explicit. Next: partitioned snapshots and bounded update work, then native scale/branch-switch/SMB/cloud tests on that binary. No all-language semantics or arbitrary-size guarantee. |
| Release assurance | Native source gates; earlier packages use older binaries; no Authenticode signing | Repackage only after gates, validate that artifact and identify signing status. Source integration and evidence publication are separate from a signed public release. |

## Implemented boundary

Only concept/task ranking changes. Exact search/symbol ordering remains stable.
Candidate scans remain bounded to 128 and output to 32 files before packing.
Source hashes, current admission/ignore rules, redaction and truncation remain
authoritative. The filename contribution uses complete words from the basename,
not partial strings or arbitrary parent directory names. Repeated query words
cannot multiply this filename contribution. BM25 remains a heuristic score,
not calibrated confidence. The five-line window uses a 12-term bit mask and
bounded source size; no embeddings, model, network or compiler call is added.

The live harness records path-free aggregate diagnostics; arguments, source and
raw model events are not copied into those aggregates. Concurrent calls sharing
a tool name retain distinct pending call IDs. Native results can omit `isError`;
the diagnostic accepts that normal shape and still detects explicit failures.

## Offline outcome

The development trial is preserved separately. After the candidate was frozen,
the full 65-task run at the same 1,200-token budget gives:

| Metric | Previous frozen binary | Candidate |
|---|---:|---:|
| All hit@1 | 40.00% | 44.62% |
| All gold recall@10 | 79.23% | 80.77% |
| All MRR | 0.543 | 0.579 |
| Dev hit@1 (37 tasks) | 37.84% | 43.24% |
| Dev gold recall@10 | 77.03% | 77.03% |
| Test hit@1 (28 tasks) | 42.86% | 46.43% |
| Test gold recall@10 | 82.14% | 85.71% |
| CLI latency p50 | 193.6 ms | 188.5 ms |
| CLI latency p95 | 241.9 ms | 245.5 ms |

There are individual ranking losses. This test split was exposed by earlier
work and is not a fresh blind holdout. Timing overlaps other deterministic
checks and is not a controlled speed claim. Neither retrieval acceptance target
is met. The baseline is Findex itself; these localization results do not score
Graphify or codebase-memory.

Native stable/MSRV pass 188 Rust tests; 49 offline Python tests, 29 security
checks, six real rust-analyzer cases, 115 unchanged navigation hit lists and
460 context-budget reports (zero overruns) pass. The first security invocation
used an absent corpus directory and did not complete; it is retained separately
from the successful rerun. Memory observations in navigation controls are final
RSS samples, not peak/scale measurements.

[Final localization and gate evidence](../../validation/results/20261029-locality-final/verification.json),
[development trial](../../validation/results/20261028-locality-dev-v1/localization.json),
[security rerun](../../validation/results/20261029-locality-security-v2/security.json),
[compiler fixtures](../../validation/results/20261029-locality-semantic/results.json).

## Live protocol

At most six new starts: the same two public issues, three tools, one repetition;
official Codex ChatGPT subscription, current native model profile, no paid API
or subagents. Fresh red/green upstream oracles precede frozen equal-source arms.
SCIP setup is not selected in this campaign and cannot be advertised as preserved
semantic functionality. Source edits run in native workspace-write MXC;
independent Windows host grading uses the unchanged public reproduction. Model
compilation is unsupported and explicitly left to the host, equally for all arms.

The old study, including its timeout, remains unchanged. A new small public-task
pilot is exploratory. Compare successes per attempt first, then total/cache/
uncached/output tokens and preparation/model/grader time separately. Missing
usage prevents whole-study token ranking. No retry or model start above the cap.

## Renewed live outcome

All three tools pass both supplied native host regressions in exactly six starts.
No timeout occurs. Findex uses 494,954 total tokens versus Graphify 723,857
(31.6% fewer) and codebase-memory 419,689 (17.9% more). Findex uncached input is
60,608 versus codebase-memory 65,033 (6.8% fewer); cache weighting and monetary
cost cannot be inferred. Model wall time is 80.951 / 71.073 / 70.904 seconds for
Findex / Graphify / codebase-memory. Setup is 17.828 / 29.736 / 29.708 seconds,
with no SCIP preparation. Preparation + model + grading totals are close;
the two-task pilot is not a controlled universal speed result.

This improves observed completion over the earlier timeout campaign, without
establishing its cause or sustained reliability. It ties success, beats Graphify
on tokens, and still loses to codebase-memory on total tokens and model time.
[Inputs, component costs, six patches and limitations](../../validation/results/20261029-locality-bugfix-live/NOTE.md).
