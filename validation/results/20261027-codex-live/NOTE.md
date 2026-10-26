# Real Codex subscription pilot — 2026-10-27

Six sequential official Codex CLI sessions, no subagents, no paid API route.
The user renewed live-test authorization after the earlier usage-limit stop.
Six earlier starts plus these six starts equal the authorized twelve-session
ceiling; this is a new dated study, not a retry hidden in the old study.

Read-only tasks were selected by a fixed hash from the 55 new queries:
requests `JSONDecodeError` exact definition and zod `native enum test` terms.
Each tool gets both tasks once. The task selection is saved in `manifest.json`.

| Tool | Valid/correct | Successful MCP calls | Input | Cached input (included in input) | Output | Input + output |
|---|---:|---:|---:|---:|---:|---:|
| Findex | 2/2 | 2 | 149210 | 97920 | 317 | 149527 |
| Graphify | 2/2 | 3 | 216790 | 155904 | 490 | 217280 |
| Codebase-memory | 2/2 | 8 | 591285 | 515968 | 1100 | 592385 |

Findex's total client-reported tokens are 31.2% lower than Graphify and 74.8%
lower than codebase in **these two tasks**. Uncached input is 51290, 60886 and
75317 respectively (15.8% and 31.9% lower for Findex). Cache and call-count
differences matter. These figures include the Codex client/system context,
not just navigation responses. Cached input is not added a second time;
reasoning output is a subset of output and is not added again.

Profile: gpt-6.1-sol, high reasoning, priority, kept from the user's existing
configuration. `codex login status` reported ChatGPT login before execution;
the runner strips API/provider environment routes and forces ChatGPT/OpenAI.
Official Codex owns authentication; no auth files were opened or copied.
Only the selected server's read tools were exposed. Plugins and project
instructions were disabled equally for all arms. Findex was told to use
compact and codebase to use tree/anchored exact lookup, with source verification
allowed for everyone. All answers were checked against frozen file/line anchors.

**Scope:** navigation only, two tasks, one repetition per arm, all tools tied
on success. No variance estimate, code-edit outcome, architecture coverage,
general superiority, monetary saving or exact subscription-billing claim is
supported. At pilot time the matrix-only raw-tool-token gate failed against
codebase tree; this successful live pilot did not replace that gate. The later
final format passes the aggregate local gate, with corpus/control losses
disclosed in `../20261027-final-comparison/`. These live sessions used the
earlier GNU matrix-only binary, not the final MSVC/zero-sync executable.

`live-results.json` journals each attempt before process spawn and stores usage,
success and final answers. `summary.json` includes limits and the cumulative
attempt count. `raw/live/` holds local ignored transcripts. No further live
sessions are started automatically; a stopped/pending attempt prevents retry.
Claude was explicitly excluded; its existing integration evidence is separate.
