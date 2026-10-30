# Live issue-localization re-run with ranked-files line (R11)

Same eight issues, prompt, model (Claude Haiku, subscription), 20-turn cap and scoring as
`20261030-localization-live`; Findex arm only (`--arms findex`), run on a release build that adds one
line to concept output: `ranked files, most likely first: a, b, c, d, e` (the five best-ranked files in
rank order; packing otherwise groups snippets by kind, so the rank was not visible).

| | Correct first file | Valid sessions | Mean turns | Tokens per session |
|---|---:|---:|---:|---:|
| R10 Findex | 4/8 | 5/8 | 13.1 | 420,352 |
| R11 Findex | 6/8 | 7/8 | 5.0 | 143,014 |

requests-3844 and zod-1046 changed from turn-cap failures to correct. Remaining losses:
ripgrep-2122 (the ranked line put the gold file `hiargs.rs` first, but the model answered
`cli/src/lib.rs`, `wtr.rs`, `process.rs` after judging the snippets) and zod-2184 (gold file ranks 16th
offline, outside the listed five).

## Limits

- The change was designed after reading the R10 failures on these same issues, so this is a
  regression check with a visible gain, not an independent test. Paired outcomes are 2 up, 0 down,
  6 unchanged on eight issues.
- Rivals were not re-run; the R10 rival numbers are from the earlier build of the study and
  unchanged. Offline localization numbers are unchanged (JSON output has no new line).
- One repetition, Haiku only, Windows only.
