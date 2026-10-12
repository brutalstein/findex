# Claude Code A/B/C/D/E results

Status: **EXECUTED (Run 3, 120/120 sessions). Result: negative.** With Haiku,
no Findex arm saved tokens; `findex_context` (D) used **+31 % total tokens**
vs built-ins only (A), 95 % CI excluding zero.

## Run 3 — `results/20261012T105254Z-397bf78730ba` (code `397bf78`)

8 tasks × 5 arms × 3 repetitions, all EXECUTED, none BLOCKED. Estimated
client-side spend 4.77 USD (subscription; not an invoice). Statistics:
`ab_stats.json` (paired by task and repetition, bootstrap 10,000 draws,
seed 11; `harness/ab_stats.py`).

### Pooled over all 8 tasks (24 pairs per arm)

| Arm vs A | Δ total tokens per session | 95 % CI | Relative | Δ turns [95 % CI] | Correct (arm / A) |
|---|---|---|---|---|---|
| B (4 tools, deferred) | +15.2k | [−0.2k, +30.8k] | +20.8 % | +0.88 [−0.08, +2.00] | 24/24 / 24/24 |
| C (B + guidance) | +14.0k | [+2.9k, +25.8k] | +19.2 % | +1.00 [+0.21, +1.79] | 24/24 / 24/24 |
| **D (`findex_context`)** | **+22.7k** | **[+8.7k, +37.6k]** | **+31.1 %** | **+1.33 [+0.33, +2.33]** | **23/24** / 24/24 |
| E (no Grep/Glob) | +51.1k | [+15.1k, +93.4k] | +70.0 % | +3.25 [+1.08, +5.54] | 18/24 / 24/24 |

### Per task (mean of 3; tokens in thousands; correct/3; sessions that called Findex)

| Task | A | B | C | D | E |
|---|---|---|---|---|---|
| n1 named file | 26k 3/3 | 26k 3/3 fx0 | 27k 3/3 fx0 | 27k 3/3 fx0 | 24k 3/3 fx0 |
| n2 no repo | 13k 3/3 | 13k 3/3 fx0 | 13k 3/3 fx0 | 13k 3/3 fx0 | 12k 3/3 fx0 |
| t1 find function | 45k 3/3 | 63k 3/3 fx3 | 64k 3/3 fx3 | 59k 3/3 fx3 | 128k **1/3** fx3 |
| t2 callers | **28k** 3/3 | 104k 3/3 fx3 | 58k 3/3 fx3 | 76k 3/3 fx3 | 193k **1/3** fx3 |
| t3 concept | 49k 3/3 | 72k 3/3 fx0 | 75k 3/3 fx2 | 100k **2/3** fx3 | 175k **1/3** fx3 |
| t4 trace | 140k 3/3 | 140k 3/3 fx0 | 198k 3/3 fx2 | 195k 3/3 fx2 | 202k 3/3 fx3 |
| t5 fix bug | 152k 3/3 | 150k 3/3 fx0 | 119k 3/3 fx1 | 133k 3/3 fx2 | **74k** 3/3 fx3 |
| t6 feature | 131k 3/3 | 137k 3/3 fx0 | 142k 3/3 fx1 | 162k 3/3 fx1 | 185k 3/3 fx3 |

D minus A per task (k tokens, 95 % CI): t1 +14.8 [+14.4, +15.1]; t2 +48.1
[+14.3, +100.1]; t3 +51.0 [−5.4, +115.9]; t4 +55.3 [+22.2, +81.5]; t5 −19.5
[−45.0, +6.6]; t6 +30.4 [+5.2, +43.1]. The only task where any Findex arm
saved tokens significantly is t5 (C −33.0k [−41.2, −20.9]; E −78.5k
[−102.8, −45.7]).

### Why D did not save tokens (diagnosis, not tuned on)

- **Tool selection mostly worked.** In all 6 D sessions on t1/t2, a Findex
  tool was the first evidence tool. `findex_context` was called directly in
  3 of 6; in the other 3 the model first spent a ToolSearch turn (2× to load
  `findex_find_symbol`, 1× before `findex_context`).
- **The model did not trust the answer.** On t2, every D session followed
  `findex_context` with 1–6 Grep calls. Reproducing the call on the same
  workspace shows the report was complete and correct: definition, 24 call
  sites with enclosing functions, 1 reference, "complete for the indexed
  files", ~790 tokens.
- **The built-in baseline is strong.** Arm A answered t2 with a single Grep
  in 2 turns (28k tokens) every time. When one Grep already answers, any
  extra tool (or a ToolSearch turn) can only add a turn.
- **Without Grep (E)**, Haiku searched for substitute tools and failed 6 of
  18 navigation sessions. Findex is not a sufficient replacement for Grep for
  this model.
- The one-session pilot of Run 1 (D: 2 turns on t1/t2) did not replicate;
  run-to-run variance per cell is large.

Negative controls: no arm called Findex on n1/n2, and the upfront
`findex_context` schema cost about +1k tokens per session (n1 26k → 27k).

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

Two later attempts at `fc6c4e7` (`20261012T104630Z`, `20261012T104841Z`)
measured nothing: one could not see the corpora from the operator's terminal
(the Claude app virtualizes `%LOCALAPPDATA%`), the other was BLOCKED on the
same expired sign-in. A third attempt at `fc6c4e7` ran 5 sessions and then
crashed copying ripgrep's untracked `target/` (MAX_PATH); fixed in `397bf78`
by copying only tracked files. Run 3 started fresh after that fix. None of
these partial directories is committed.

## Conclusion

Token savings with Claude Code are **refuted for Haiku 4.5 on these tasks**,
not merely undemonstrated. The single-session pilot (Run 1) suggested fewer
turns for D/E; Run 3 with three repetitions does not reproduce it. Findex's
measured value is in retrieval quality (RETRIEVAL_RESULTS), not in reducing
this model's token use.
