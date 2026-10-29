# Claude Code live round R4, 2026-10-30

Claude Haiku (`--model haiku`) through the Claude.ai Pro subscription; no API key.
Repaired harness (`tool_comparison_live.py --tasks 4`): single-definition tasks chosen
by rule, an invalid session no longer halts the study, answers are parsed tolerantly,
and a Findex preflight runs before any session is spent. 12 sessions, one repetition per arm.

Tasks (exact-symbol, one expected location, exactly one declaration in the frozen
corpus by `git grep`): `fmt:KeyMatcher`, `requests:HTTPProxyAuth`,
`ripgrep:GitignoreBuilder`, `fmt:concat`. Frozen manifest copied from
`20261030-claude-live-haiku-b`.

| Tool | Valid | Correct | Total tokens | Per session |
|---|---:|---:|---:|---:|
| Findex | 4/4 | 4/4 | 315,804 | 78,951 |
| Graphify | 4/4 | 4/4 | 323,963 | 80,991 |
| codebase-memory | 3/4 | 3/4 | 385,086 | 96,272 |

Tokens are fresh input + cache creation + cache read + output as reported by Claude
Code. The original `live-usage-summary.json` showed 0 tokens because it read the wrong
field; it was regenerated from the saved sessions. Raw transcripts stay in `raw/`.

- Every session spent a `ToolSearch` turn (tools are deferred) and three turns in total,
  about 72-78k tokens, mostly cache reads. The differences above come from extra `Read`
  or `Grep` turns, not from retrieval quality: Findex needed one extra `Read` (`fmt:concat`),
  Graphify one (`HTTPProxyAuth`), codebase-memory two.
- The one invalid session is codebase-memory on `requests:HTTPProxyAuth` (its only MCP call
  returned an error); it counts as a failure.
- The harness prompt steered Findex sessions to the deferred `findex_find_symbol`, although
  `findex_context` is loaded at session start. The R5 round fixes the hint.

Limits: four tasks, one repetition, one small model. A 2.5% token gap is inside run-to-run
noise; this round shows parity on success and tokens, not superiority.
