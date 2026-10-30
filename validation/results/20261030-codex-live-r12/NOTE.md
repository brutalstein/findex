# Codex subscription round on fresh definition tasks (R12)

Codex CLI through a ChatGPT subscription (no API key), model `gpt-5.6-luna`, reasoning effort `low`,
no priority tier, Windows, one repetition. Four rule-selected single-definition tasks, skipping the
eight used by R4-R6 (`--tasks 4 --skip-tasks 8`): ripgrep `NoIgnoreFiles`, ripgrep `NoIgnoreMessages`,
fmt `utf8_system_category`, requests `TLSServer`. Three tools (Findex, Graphify, codebase-memory), 12
sessions, an invalid session counts as a failure. The earlier luna pilot
(`20261030-codex-live-luna`) was blocked by the usage limit; this round ran to completion.

| Tool | Correct | Valid | MCP calls | Tokens per session | Uncached input per session |
|---|---:|---:|---:|---:|---:|
| Findex | 4/4 | 4/4 | 6 | 62,825 | 20,595 |
| Graphify | 3/4 | 4/4 | 7 | 165,062 | 22,916 |
| codebase-memory | 3/4 | 3/4 | 11 | 181,407 | 26,728 |

Findex used 62% fewer total tokens than Graphify and 65% fewer than codebase-memory. Most of that gap
is cached input; measured on uncached input the gap is 10% and 23%. The misses: Graphify answered
ripgrep `NoIgnoreFiles` wrongly (valid session, wrong answer); codebase-memory's requests `TLSServer`
session was invalid (counted as a failure).

## Limits

Four tasks, one repetition, one cheap model, Windows only. Four single-definition lookups: this lane does
not exercise the ranked-files line or issue localization. Token totals include cache reads. Not evidence
of general superiority or monetary savings.
