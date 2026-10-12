# Claude Code A/B/C/D/E results

Status: **PARTIAL — the full study is BLOCKED.** No statistically supported
token claim can be made from this campaign.

## Design (`validation/harness/claude_ab.py`)

| Arm | Tools | Findex binary |
|---|---|---|
| A | built-ins only (Read, Grep, Glob, …) | — |
| B | built-ins + the 4 original Findex tools | frozen main `caf33fa` |
| C | B + `--append-system-prompt` tool-use guidance | frozen main `caf33fa` |
| D | built-ins + 5 tools incl. `findex_context` (`alwaysLoad`) | this branch |
| E | Read/Edit + Findex tools; Grep/Glob denied | this branch |

- Tasks: t1 find a function, t2 identify callers, t3 explain a concept, t4
  cross-file trace, t5 bug localisation, t6 small feature with `cargo test`;
  negative controls n1 (answer is in a named file) and n2 (no repository
  needed).
- Model: Haiku (`claude-haiku-4-5`), Claude Code 2.1.281, subscription
  account, OAuth. No API key, no pay-as-you-go.
- Isolation: `CLAUDE_CODE_DISABLE_CLAUDE_MDS=1`, `Skill` denied,
  `--setting-sources project,local`. Order counterbalanced per repetition.
- Judging by scripts and predefined criteria, never by a model.

## Validity finding (affects all earlier runs)

Before this campaign, every A/B session loaded the operator's personal
`~/.claude/CLAUDE.md` and skills (confirmed by probe). The earlier A/B numbers
in `FINAL_VALIDATION_REPORT.md` (+5.5 % tokens, 3/18 Findex use) were
therefore measured under uncontrolled instructions. They are kept, labelled,
and not compared with vNext numbers.

## Run 1 — `results/20261012T014859Z-5f888fe5f970` (code `5f888fe`)

19 sessions executed (repetition 0 of n1, n2, t1 for all arms; t2 for B–E).
The rest are BLOCKED: the Claude plan session limit was reached ("You've hit
your session limit"). One session per cell: **these are observations, not
estimates.**

| Task | Arm | Correct | Turns | Tool calls | Findex calls | ToolSearch | Total tokens |
|---|---|---|---|---|---|---|---|
| n1 | A/B/C/D/E | 5/5 | 2 | 1 (Read) | 0 | 0 | 45.5k–48.1k |
| n2 | A/B/C/D/E | 5/5 | 1 | 0 | 0 | 0 | 22.5k–23.8k |
| t1 | A | yes | 3 | 2 (Grep) | 0 | 0 | 71.4k |
| t1 | B | yes | 3 | 2 | 1 | 1 | 71.9k |
| t1 | C | yes | 3 | 2 | 1 | 1 | 72.1k |
| t1 | D | yes | 2 | 1 | 1 | 0 | 48.5k |
| t1 | E | yes | 2 | 1 | 1 | 0 | 45.8k |
| t2 | B | recall 0.92 | 6 | 5 | 2 | 1 | 161.6k |
| t2 | C | recall 0.92 | 6 | 5 | 2 | 1 | 135.0k |
| t2 | D | recall 1.00 | 2 | 1 | 1 | 0 | 49.9k |
| t2 | E | recall 1.00 | 2 | 1 | 1 | 0 | 47.5k |
| t2 | A | BLOCKED | — | — | — | — | — |

Observations:

- **Tool selection.** With `findex_context` loaded upfront (D, E), the model
  called it first on t1 and t2 and answered after one call. With deferred
  tools (B, C), every Findex use was preceded by a `ToolSearch` turn.
- **Negative controls.** No arm called Findex on n1 or n2, and the token cost
  of having D's upfront tool was within ±2.6k of A on the same task
  (n1 48.1k vs 47.2k; n2 23.8k vs 23.4k), i.e. small but not measured with
  repetitions.
- **C (guidance prompt) did not remove the ToolSearch turn**, so prompting
  alone is not a substitute for `alwaysLoad`.

## Run 2 — `results/20261012T100733Z-96295ee75261` (code `96295ee`)

All 51 sessions failed before any model call: "Failed to authenticate: OAuth
session expired and could not be refreshed". The harness of that commit
scored them as wrong answers; they were reclassified BLOCKED
(`blocked_session()`), and the harness now stops on this condition. **No
measurement.** The run was stopped by the operator.

## What is required to complete

1. The account owner signs in again (`/login` or `claude auth login`).
   Findex never handles credentials.
2. Run: `validation/scripts/run-claude-ab.ps1 -Reps 3 -Arms 'A,B,C,D,E'`
   (8 tasks × 5 arms × 3 reps = 120 sessions; the run resumes with
   `--resume` if a plan limit stops it).
3. Report per task: paired token difference vs A with 95 % CI, success
   rate, Findex use rate, first-evidence tool call.

Until then: **token savings and task-success differences are NOT
demonstrated.** The single-session observations are consistent with fewer
turns for D/E on navigation tasks, which is the mechanism vNext targets.
