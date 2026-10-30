# Fresh-holdout localization baseline (v2) — 2026-10-30

Dataset `validation/golden/issue_localization_v2.jsonl` (55 tasks; sha256 prefix `b3390b79`), frozen in
commit `3046902` before any system was scored on it. Built by `issues.py --exclude issue_localization.jsonl
--include-merges`: real closed issues, query = reporter's issue title, gold = non-test source files changed
by the closing commit, search at the parent commit. Disjoint from the 65 exposed v1 tasks.

Composition: requests 19, ripgrep 25, fmt 11, zod 0 (zod had no further qualifying closing commits).
Languages are therefore Python, Rust and C++ only. Dev/test split by issue-number parity (28 / 27), as in v1.

Systems: git grep words, frozen `main` lexical search (caf33fa), and `findex context` from release
build `6528a1423f14` (the binary before the report-routing change). 8,000-token rendered budget
(the harness default), JSON consumption through `ranked` + evidence paths.

Result for `findex context`, all 55: hit@1 0.364, hit@3 0.691, hit@10 0.745, gold recall@10 0.706,
MRR 0.529. Exposed v1 with the same binary: hit@1 0.585, recall@10 0.869. The 22-point hit@1 gap is the
optimism of tuning on a set that was examined over many rounds, not a regression.

Protocol: per-task rows were read for the dev half only. The test half (n=27) was read in aggregate only
and is scored once per candidate binary (this baseline and the report-routing candidate). Standard error at
n=55 is about +/-6 points; the dev and test halves differ by 23 points (0.25 vs 0.48), which shows how wide
the interval is at this size. No model or API calls.
