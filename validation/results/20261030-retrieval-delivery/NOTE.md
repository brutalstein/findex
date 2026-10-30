# Codex delivery: plugin install and paired bugfix pilot (2026-10-30)

Run by Codex after the retrieval-integrity checkpoint; documented here from the saved JSON.

- **Installation (`installation.json`):** `findex@findex-local` v0.2.0 installed and enabled from the candidate
  binary (SHA-256 `6528a142...c35271b68`); MCP config written as BOM-less UTF-8; a live MCP probe returned the
  `enqueue_event` definition from the working tree. Status PASS.
- **Paired bugfix pilot (`live/`):** four authorized Codex subscription starts, Findex only, old binary
  (`6caac637...2eee59f47`, session-start executable) versus candidate, on two reused tasks
  (`requests-5223`, `ripgrep-2659`). Codex profile from its config: `gpt-5.6-sol`, high effort, priority tier
  (not the cheapest model). No competitor servers, no retries, no fallback.

| Arm | Starts | Valid | Correct | Tokens (complete sessions only) |
|---|---:|---:|---:|---:|
| Baseline | 2 | 1 | 1 | 518,177 (requests-5223) |
| Candidate | 2 | 2 | 2 | 445,312 (requests-5223), 483,589 (ripgrep-2659) |

On the one task where both arms have complete usage (`requests-5223`) the candidate used 14% fewer tokens.
The `ripgrep-2659` baseline session was invalid (missing usage record), so the harness stopped as designed
and no whole-study token comparison exists.

## Limits

Two tasks, one repetition, one model. This compares two Findex builds, not Findex against Graphify or
codebase-memory. Success is the regression test only, not the full upstream suite. Not evidence of general
superiority or monetary savings. The candidate differs from the baseline by the JSON ranking, semantic
stale-evidence reporting and watcher filtering, none of which the two tasks are designed to isolate.
