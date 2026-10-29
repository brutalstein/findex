# Codex subscription pilot on the cheapest model (2026-10-30): blocked

Authorization (user, 2026-10-30): subscription only, no paid API, cheapest
subscription model, at most six sessions. Claude was not run: `claude auth
status` reports `authMethod: none` with `apiKeySource: apiKeyHelper`, which the
harness correctly refuses (it requires a claude.ai subscription sign-in).

Profile: `gpt-5.6-luna` ("Older fast and efficient model" in the Codex model
catalog), reasoning effort `low`, no priority service tier. Tasks, chosen by
SHA256(`live-luna-v1` + id) among the new-55 exact-symbol queries of fmt and
ripgrep, excluding the 2026-10-27 tasks: `fmt:parse_arg_id:exact`,
`ripgrep:NoIgnoreFiles:exact`. Frozen manifest and setups are copied from
`../20261027-codex-live/`; only the live task ids and the codebase-memory
project names differ (indexed persistently for this study).

Result: **0 valid sessions.** The first session (Findex) failed immediately with
the ChatGPT usage-limit message ("try again at 2:21 AM"); no model output, no
MCP call, no usage record. The harness stopped, as designed: no retry, no
fallback. One of six authorized starts is used. No claim of any kind follows
from this directory.

To continue after the quota resets, rerun the same command with the same
`--out`; the invalid attempt is preserved and not retried (the Findex arm of the fmt task stays empty), and the remaining five starts
stay available.
