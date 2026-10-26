# Findex vs Graphify vs codebase-memory-mcp (2026-10-26)

Plan and measurement contract, frozen before running:
`docs/engineering/CODEX_AND_COMPARISON_PLAN.md`. Harness:
`validation/harness/tool_comparison.py` (self-checks in
`test_tool_comparison.py`). Native Windows 11, no model calls for the local
part.

## Local retrieval (`local-summary.json`, `local-results.json`, `manifest.json`)

60 queries: 15 per corpus (requests, fmt, ripgrep, zod), drawn
deterministically from the **held-out** split of the golden set, balanced by
query type (`manifest.json` records the IDs and SHA-256 of the golden set).
Same candidate limit (10) and output budget (2,000 `o200k_base` tokens) for
every tool. Errors and misses count in the denominators.

| Tool (as installed) | file hit@1 | span hit@1 | hit@10 | MRR | mean visible tokens (clipped) | p50 latency | over budget |
|---|---|---|---|---|---|---|---|
| **Findex** `findex_find_symbol` / `findex_search` (MCP) | **0.967** | **0.967** | **1.000** | **0.983** | 419 | **8 ms** | 0 |
| Graphify, code-only AST mode (CLI) | 0.650 | 0.567 | 0.717 | 0.676 | 1,466 | 904 ms* | 20 |
| codebase-memory-mcp (MCP) | 0.633 | 0.433 | 0.700 | 0.646 | **355** | 18 ms | 0 |

\* Graphify's CLI latency includes process start-up; its MCP surface was not
timed.

These token counts measure visible output after the 2,000-token cutoff.
Mean actual returned output was 418.55 tokens for Findex, 354.63 for
codebase-memory-mcp, and 1,919.10 for Graphify; Graphify's clipped mean was
1,466.07. The 20 over-budget Graphify responses were clipped before scoring.

Pre-registered gate: Findex had to beat **each** competitor on both accuracy
and tokens. It beats both on accuracy and beats Graphify on tokens, but
codebase-memory-mcp returns fewer tokens per answer (355 vs 419). The gate is
**not passed**.

Limits, in addition to those in `local-summary.json`:
- The golden set was written by the Findex author, and Findex was developed
  on these corpora (the held-out split was not tuned on, but the task style
  was). Competitors ran with default settings and no tuning. This favours
  Findex.
- The tasks are definition lookups (exact names via `findex_find_symbol`,
  word queries via `findex_search`), which Findex is built for. Concept questions, call graphs and impact
  analysis were not compared.
- Graphify ran in local AST mode, not its LLM semantic extraction.
- Context tokens are a cost proxy, not session usage or money.
- The historical manifest froze query hashes and corpus commits, but did
  not record corpus-content hashes or tool versions/executable hashes. The
  current harness requires those identities at preparation and run time and
  rejects measured output directories. Preserve this historical directory;
  to measure again, prepare a new directory with `--findex`, `--cbm` and
  `--graphify`. Current identities cannot be attributed to this past run.

## Live pilot with Codex and Claude (`live-*.json`, `codex-native.json`)

Not conclusive, and not used for any claim.
- Codex: four initial attempts were invalid because the harness did not
  approve read-only MCP calls; the fourth was interrupted.
  `live-invalid-permission-trials.json` archives that prefix (`started: 4`,
  three completed records). It is included in the cumulative `started: 6`
  in `live-results.json`, not four additional attempts.
- Of the next two attempts, Findex successfully returned the correct
  requests `__bool__` definition (`src/requests/models.py:730`). Graphify's
  attempt exited 1 with no recorded usage and stopped the pilot; its raw
  transcript reports a usage-limit failure. This is an invalid trial, not
  a measured retrieval miss. No valid codebase-memory trial followed.
- Claude: the arm was BLOCKED by the subscription gate in that environment.
  No Claude session started.
- In total, six of the 12 authorized attempts were started, including the
  four invalid permission attempts. The overlapping archive must not be
  added to the cumulative count. Missing trial usage prevents a complete
  session-token total; neither subscription billing nor money spent can be
  inferred from the missing usage. There was no fallback after the stop.

`codex-native.json` does show that the Codex app-server loads the Findex
plugin, lists all 8 tools, and returns a source-verified `findex_context`
answer. `codex-install.json` records the local plugin installation.

Raw per-query outputs and session transcripts stay local (`raw/`, ignored by
Git). Local user paths are replaced by `<repo>`/`%USERPROFILE%`.
