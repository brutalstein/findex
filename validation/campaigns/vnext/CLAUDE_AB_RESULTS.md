# Claude Code A/B/C/D/E results

Status: **EXECUTED.** Run 3 (code `397bf78`) was negative: `findex_context`
used +31 % tokens because Haiku re-checked every report with Grep. After the
trust rule and coverage line (`ac4ea21`), **Run 4 is positive**: D used
**−23.7 % tokens on held-out tasks** (95 % CI excluding zero) and −15.9 %
over all 12 tasks, with 36/36 correct.

## Run 4 — `results/20261016T105620Z-ac4ea21dd692` (code `ac4ea21`)

What changed since Run 3 (designed from the Run 3 diagnosis, not tuned on
Run 4 data):
- every `findex_context` report states, per name, that all indexed files
  containing it were scanned and every code occurrence is shown, with the
  number of omitted comment lines. Rendered only when nothing was truncated.
  The claim was verified against `git grep -w` for 279/279 golden names in 7
  corpora (`results/20261016T105518Z-ac4ea21dd692/coverage/`);
- the MCP instructions and the tool description carry a trust rule: a
  complete report makes Grep redundant; if unsure, ask Findex again; if a
  file contradicts the report, trust the file.

Arms: A (built-ins only), D (Findex incl. `findex_context`, rule in MCP
instructions only), F (D + the same rule as `--append-system-prompt`).
12 tasks × 3 arms × 3 repetitions = 108 sessions, all EXECUTED. Estimated
client-side spend 3.46 USD (subscription; not an invoice). Four tasks are
**held out** (h1–h4: fmt C++ callers of `to_ascii`, requests Python callers
of `default_hooks`, zod TS definition of `CatchallOutput`, pybind11 C++
definition of `Dupe1`), chosen by fixed rules written into `claude_ab.py`
before any Findex run on them. Statistics: `ab_stats.json` (paired by task
and repetition, bootstrap 10,000 draws, seed 11).

| Group (pairs) | Arm | Δ tokens / session vs A | 95 % CI | Relative | Δ turns [95 % CI] | Correct (arm / A) |
|---|---|---|---|---|---|---|
| **held-out h1–h4 (12)** | **D** | **−15.1k** | **[−21.4k, −8.7k]** | **−23.7 %** | **−1.50 [−2.67, −0.58]** | **12/12 / 12/12** |
| held-out h1–h4 (12) | F | −14.9k | [−21.1k, −8.6k] | −23.4 % | −1.50 [−2.67, −0.58] | 12/12 / 12/12 |
| dev t1–t6 (18) | D | −24.7k | [−50.0k, +1.1k] | −15.2 % | −0.33 [−1.50, +0.83] | 18/18 / 18/18 |
| dev t1–t6 (18) | F | −24.7k | [−54.2k, +4.1k] | −15.2 % | −0.94 [−2.17, +0.39] | 17/18 / 18/18 |
| negative n1–n2 (6) | D | +0.8k | [+0.6k, +1.0k] | +2.2 % | 0 | 6/6 / 6/6 |
| negative n1–n2 (6) | F | +1.0k | [+0.8k, +1.3k] | +3.0 % | 0 | 6/6 / 6/6 |
| **all 12 tasks (36)** | **D** | **−17.3k** | **[−31.2k, −4.0k]** | **−15.9 %** | −0.67 [−1.39, +0.03] | **36/36 / 36/36** |
| all 12 tasks (36) | F | −17.1k | [−32.6k, −2.6k] | −15.8 % | −0.97 [−1.69, −0.22] | 35/36 / 36/36 |

Per task (mean of 3; correct/3; `g` = built-in Grep/Glob calls summed over 3 sessions):

| Task | A | D | F |
|---|---|---|---|
| h1 callers C++ | 73k 3/3 4.7 turns g11 | **49k** 3/3 2.0 g0 | 49k 3/3 2.0 g0 |
| h2 callers Python | 64k 3/3 4.0 g9 | **49k** 3/3 2.0 g0 | 49k 3/3 2.0 g0 |
| h3 definition TS | 55k 3/3 2.7 g5 | **48k** 3/3 2.0 g0 | 49k 3/3 2.0 g0 |
| h4 definition C++ | 63k 3/3 2.7 g4 | **49k** 3/3 2.0 g0 | 48k 3/3 2.0 g0 |
| t1 find function | 87k 3/3 3.7 g8 | **49k** 3/3 2.0 g0 | 49k 3/3 2.0 g0 |
| t2 callers | 135k 3/3 5.3 g13 | **50k** 3/3 2.0 g0 | 51k 3/3 2.0 g0 |
| t3 concept | 96k 3/3 4.3 g6 | 110k 3/3 4.3 g0 | 102k 3/3 4.0 g0 |
| t4 cross-file trace | 203k 3/3 6.3 g6 | 246k 3/3 8.3 g6 | 204k **2/3** 6.3 g2 |
| t5 fix bug | 210k 3/3 7.7 g10 | 193k 3/3 9.0 g15 | 202k 3/3 8.3 g12 |
| t6 feature + test | 243k 3/3 9.0 g14 | 178k 3/3 8.7 g11 | 219k 3/3 8.0 g5 |
| n1 named file | 47k 3/3 2.0 | 48k 3/3 2.0 | 49k 3/3 2.0 |
| n2 no repository | 23k 3/3 1.0 | 24k 3/3 1.0 | 24k 3/3 1.0 |

D minus A per task with CIs excluding zero: h1 −23.8k [−24.6, −23.3]; t1
−38.5k [−46.3, −23.0]; t2 −85.0k [−160.3, −47.0]; t6 −65.0k [−70.9, −53.7];
and one **cost**: t3 (concept question) **+14.0k [+2.4, +20.0]**. h2–h4 are
negative with CIs touching zero.

### Mechanism (Run 3 → Run 4, arm D on t1/t2)

| | Run 3 (`397bf78`) | Run 4 (`ac4ea21`) |
|---|---|---|
| sessions that called Grep/Glob after Findex | 6 / 6 | **0 / 6** |
| ToolSearch calls | 3 | **0** |
| turns per session | 3–8 | **2 in every session** |

### Reading

- On navigation tasks (definitions, callers), with the coverage line and trust
  rule, Haiku answers from one `findex_context` call in 2 turns and does not
  re-search. This holds on the four held-out tasks in three other languages.
- Concept questions (t3) cost more with Findex; cross-file trace (t4) and
  code-change tasks (t5, t6) show no significant difference.
- Putting the rule into the system prompt as well (F) adds nothing over the
  MCP instructions (D). No user or global Claude setting is needed.
- Arm A's cost varies between runs (t2: 28k in Run 3, 135k in Run 4), so only
  within-run paired comparisons are valid. Run 3 and Run 4 are not pooled.
- Run 3's numbers below were computed by `ab_stats.py` before the `groups`
  field was added; its committed `ab_stats.json` is kept unchanged.

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

Run 3 (`397bf78`): token savings were **refuted for Haiku 4.5**, because the
model repeated every Findex answer with Grep. Run 4 (`ac4ea21`), after the
coverage line and trust rule: **D saves tokens and turns on navigation tasks,
including four held-out tasks** (−23.7 %, 95 % CI [−21.4k, −8.7k] per
session), with no loss of correctness (36/36). Not demonstrated: savings on
concept questions (t3 costs +14.0k), cross-file traces and code changes;
models other than Haiku 4.5; repositories other than the 7 corpora.
