# Live issue-localization study (R10)

Claude Code `haiku` through a claude.ai Pro subscription (no API key), Windows, one repetition.
Eight held-out issues (two per corpus: fmt, requests, ripgrep, zod; test split, fixed hash order
`loc-live-v1`, never chosen by outcome). Each session gets only the issue title in a checkout of the
fix's parent commit and must name at most three source files. Scored against the gold files of the
closing commit. Arms: Findex (`findex_context`), Graphify MCP, codebase-memory MCP, and no tool
(Read, Grep, Glob only). 32 sessions, 20 turns each. An invalid session (turn cap, timeout, no
answer) counts as a failure.

## Result

| Arm | Correct first file | Valid sessions | Mean turns | Tokens per session |
|---|---:|---:|---:|---:|
| Findex | 4/8 | 5/8 | 13.1 | 420,352 |
| codebase-memory | 4/8 | 4/8 | 20.0 | 716,021 |
| Graphify | 3/8 | 6/8 | 17.6 | 657,830 |
| No tool | 3/8 | 4/8 | 21.0 | 631,956 |

Findex ties codebase-memory on accuracy and uses 33-41% fewer tokens than the other three arms
(tokens include cache reads). With eight issues and one repetition the accuracy differences are
within noise; the token gap is large but also single-run.

## What went wrong for Findex

Three Findex sessions (requests-3844, zod-1046, zod-2184) ran out of turns and one (ripgrep-2122)
answered the wrong files. Offline, `findex context` ranks the gold file first for seven of these
eight issues (zod-2184: rank 16). So the live loss was not retrieval: the output listed doc-comment
snippets grouped by kind, never said which file ranked first, and the model kept searching or
discarded the top file. R11 (`20261030-localization-live-ranked`) tests the fix.

## Setup history (kept for honesty)

- A first attempt exited with code 1 and zero usage in all 16 sessions (relative MCP config path
  with the checkout as working directory); no model usage was spent. Fixed by resolving `--out`.
- A second attempt at a 10-turn cap was stopped after 5 sessions because all three rival arms and the
  no-tool arm hit the cap, which would have measured the cap rather than retrieval
  (`20261030-localization-live-aborted-turncap10`). The cap was raised to 20 and the study restarted,
  so 37 live sessions were used in total, not 32; the 5 aborted ones are not part of any result.
- Pass/fail counts are from `live-results.json`; transcripts are under `raw/`.

## Limits

Eight issues, one repetition, Haiku only, Windows only. Rivals were given their documented query
tools and the same prompt; they were not tuned. Not evidence of general superiority or monetary
savings.
