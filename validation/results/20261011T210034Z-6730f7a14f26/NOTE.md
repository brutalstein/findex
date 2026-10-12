# Pre-fix run (commit 6730f7a)

This run measured the build **before** commit 8386913. Its performance data
documents the O(files²) FTS update defect (vscode cold index ~300 s, ~250 GB
read in both the baseline and this build). Claude Code sessions in this run
all stopped at the operator's plan usage limit; `tokens.json` and the
model-dependent checks in `compatibility.json` were reclassified to BLOCKED
after the fact (see their `post_hoc_note`). The report step crashed on that
data and was re-run with the fixed `report.py` against this directory,
keeping the start-of-run `environment.json`. Superseded by the next run
directory for all release-gate evidence.
