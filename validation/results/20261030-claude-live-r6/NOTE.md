# Claude Code live round R6, 2026-10-30

Same setup as R5 (`../20261030-claude-live-r5/NOTE.md`): Claude Haiku via the Claude.ai Pro
subscription, 12 sessions, one repetition per arm, same harness prompts, same release binary
as R5. **No Findex change since R5.** The four tasks are new (`--tasks 4 --skip-tasks 4`:
`requests:JSONDecodeError`, `ripgrep:GlobSetBuilder`, `fmt:is_set`, `requests:NoLenBoomFile`),
so this round measures whether the R5 result holds on tasks it was not tuned on.

| Tool | Valid | Correct | Total tokens | Per session | Turns (per session) |
|---|---:|---:|---:|---:|---|
| Findex | 4/4 | 4/4 | 217,601 | 54,400 | 2, 2, 3, 2 |
| Graphify | 3/4 | 3/4 | 412,255 | 103,064 | 3, 3, 5, 5 |
| codebase-memory | 2/4 | 2/4 | 437,793 | 109,448 | 5, 3, 5, 4 |

Invalid sessions count as failures (intent to treat), as in R4 and R5:

- Graphify, `JSONDecodeError`: the model never called the Graphify server (used `Grep` and `Read`).
- codebase-memory, `JSONDecodeError`: two failed MCP calls, no successful one.
- codebase-memory, `NoLenBoomFile`: no successful MCP call.

Findex answered every task with `findex_context` as its first call (no tool-search turn); one
session added a `Read`. Graphify and codebase-memory spent a `ToolSearch` turn in every session
that used them. Per-session cost is stable for Findex across R5 and R6 (54,853 and 54,400), while
the two rivals cost more on these tasks than on the R4/R5 tasks.

Pooled over R5 and R6 (eight distinct tasks): Findex 8/8, Graphify 7/8, codebase-memory 5/8.

Limits: eight tasks, one repetition, one small model, three corpora. The advantage comes mainly
from the integration (an upfront tool in Claude Code) and the harness hint that names it; it is
not evidence of better retrieval.

Harness incident: the first run crashed after session 3 because `summarize_session` assumed
every event's `message` is an object (a `system` retry event carries a string). Sessions 1 and 2
had already been scored. Session 3 (codebase-memory) was re-scored from its saved
`events.jsonl` (exit code 0 inferred from its `result` event) and is invalid on the same rule.
The parser was fixed, a regression test added, and `trial_attempt` now records any unexpected
harness exception and stops the study instead of leaving a trial pending. The run resumed at
session 4 with no repeated or discarded session.
