# Semantic identity checkpoint — 2026-10-28

This checkpoint adds source-position lookup of imported compiler/LSP identities.
It does not claim a complete runtime call graph, semantic coverage of every
language/project size, or superiority over every alternative.

## Implemented and verified

`findex graph-symbol --at <file> <line> <column>` and the existing
`findex_graph_symbol` MCP tool accept a source position. Lines are one-based;
columns are zero-based UTF-8 byte offsets. Existing exact symbol-ID calls remain
compatible. Both return the same source-backed evidence.

SCIP ranges retain their end positions and UTF-8/16/32 encoding. The resolver
checks scalar boundaries, uses end-exclusive ranges and selects strictly nested
token ranges over enclosing module ranges. Equal/crossing conflicting ranges,
unknown Unicode encodings, result caps and stale input remain explicit. Captured
LSP references are points; no token extent is invented. Relations schema 3
migrates existing identities while marking their missing encoding/ranges unknown.
Older executables refuse the newer schema rather than mutating it.

The native rust-analyzer check uses six hand-labelled cases: type alias,
shadowed receiver, outer receiver, generic trait declaration, function alias and
local closure. All six resolve to the expected definition through CLI and MCP.
The compiler produces the SCIP input; the expected definition lines are authored
separately. These are regression fixtures, not an independent broad accuracy study.

Native Windows verification:

- 186 Rust tests pass on stable MSVC and MSRV 1.85.0.
- 46 offline Python tests pass, including live authorization guards and a
  quoting-independent fish completion oracle.
- 29 black-box security checks pass, zero skipped; plugin and corrupt-cache/MCP
  privacy checks pass.
- All 115 navigation source-hit lists match the previous frozen executable.
- The measured product SHA-256 is
  `5ccdbc390cfcfbcf198437c1ca6e0f41885306cf1dad5546e65196d5693e2f8e`.

[Native compiler results](../../validation/results/20261028-semantic-position-final/results.json),
[verification](../../validation/results/20261028-semantic-position-final/verification.json),
[security](../../validation/results/20261028-semantic-security/security.json).
The earlier `20261028-semantic-position` run remains a separate successful
intermediate binary; it is not substituted for the final binary.

## Live study contract

The renewed campaign is capped at six new official Codex subscription sessions:
two real historical bugfix tasks × three tools × one repetition. No paid APIs,
external model providers or subagents are used. Source trees, reproduction
scripts, tool executables and the current model profile are frozen before starts.
Findex's Rust arm includes compiler-produced SCIP; its setup cost is recorded.

Models edit source in workspace-write native MXC. An independent native Windows
host grades each patch with the unchanged public issue reproduction. MXC private
temporary directories and MSVC linking are unsupported in this environment, so
models are explicitly told to leave those tests to the host grader. This is not
labelled a passing model-side test execution. All arms use the same arrangement.
Source-only edits and fixture hashes are checked before grading.

This is a small authored pilot with public tasks, not hidden/full upstream suites
or a statistically powered study. Token totals include repeated cached inputs;
cache reads, uncached input and output must be reported separately. Subscription
quota weighting and billed currency are not inferred from token counts. Model
wall time excludes preparation and independent grading.

The earlier five blocked starts remain invalid and are not merged into this
campaign. Current outcomes and usage belong to the
[separate live journal](../../validation/results/20261028-semantic-bugfix-live/live-results.json).

Primary results are Findex 1/2, Graphify 2/2 and codebase-memory 2/2. Findex's
Rust arm times out at 300 seconds without a patch; final token usage is missing.
The first-task Findex total is 191,315 tokens versus Graphify 288,152 and
codebase-memory 340,850 (33.6% / 43.9% fewer), but its 68.85 seconds is slower
than their 54.05 / 61.68 seconds. Setup totals are 42.09 / 30.23 / 36.01 seconds
for Findex / Graphify / codebase-memory. Whole-study token ranking is not evaluable.

One explicitly authorized diagnostic retry of Findex/Rust completes in 34.10
seconds and passes host grading, with 211,575 total tokens. It has warm caches
after native probes and is not substituted into the primary study. The first
timeout's cause is unknown because the original subprocess wrapper discarded
partial streams on timeout. A fault-tested fix now streams events/stderr to
owned files and publishes only a path-free timeout reason.

## Limits that remain

Lookup verifies matching admitted source bytes, not a new compiler run or every
dependency/configuration change. Reimport after relevant changes. Ingestion is
still capped at 16 MiB, 1,000 documents and 100,000 occurrences; a lookup examines
at most 128 candidate ranges. Ordinary task context remains lexical/syntactic;
compiler evidence is an explicit separate facility.

The previous issue-localization targets remain unmet: 40.0% hit@1 and 79.23%
gold recall@10 against 60% / 90% goals. Higher conditional Rust call precision
still comes with reduced confident coverage. No broad bugfix, latency, monetary
cost or all-project superiority is established by this checkpoint. No signed
public release, Windows 10 certification or fresh 100k/SMB/cloud soak is claimed.
