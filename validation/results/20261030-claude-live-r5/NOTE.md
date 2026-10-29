# Claude Code live round R5, 2026-10-30

Same setup as R4 (`../20261030-claude-live-r4/NOTE.md`): Claude Haiku via the Claude.ai Pro
subscription, same four single-definition tasks, 12 sessions, one repetition per arm, frozen
manifest copied from R4. Changes since R4:

- Release binary rebuilt with `bm25_score` serialized to three decimals (affects `findex_search` output only).
- The Findex hint in the harness prompt now names `findex_context` (loaded at session start)
  before the lookup tools. Graphify and codebase-memory prompts are unchanged.

| Tool | Valid | Correct | Total tokens | Per session | Turns (per session) |
|---|---:|---:|---:|---:|---|
| Findex | 4/4 | 4/4 | 219,412 | 54,853 | 2, 2, 2, 3 |
| Graphify | 4/4 | 4/4 | 298,945 | 74,736 | 3, 3, 3, 3 |
| codebase-memory | 3/4 | 3/4 | 357,677 | 89,419 | 3, 5, 3, 3 |

Findex sessions called `findex_context` directly, so none spent a `ToolSearch` turn; every
other session did. Findex uses 26.6% fewer tokens than Graphify and 38.7% fewer than
codebase-memory in this round. The Findex saving is the avoided tool-search turn (about 25k
cache-read tokens) plus no extra read in three of four tasks; one task (`fmt:concat`) still
spent a `Read`.

Run-to-run spread on the unchanged arms between R4 and R5: Graphify -7.7%, codebase-memory
-7.1% total tokens. The Findex gap is larger than that spread but remains four tasks, one
repetition, one small model. The saving comes from the integration (an upfront tool in Claude
Code), not from better retrieval; the hint change is part of the measured difference.
codebase-memory's invalid session (`requests:HTTPProxyAuth`, no successful MCP call) counts
as a failure, as in R4.
