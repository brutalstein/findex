# Findex Engineering Defect Registry

Priorities: P0 = critical security/data integrity/catastrophic functional;
P1 = important functionality or accuracy broken; P2 = significant
performance/reliability/maintainability; P3 = lower impact or documentation.

Statuses: SUSPECTED, CONFIRMED, IN_PROGRESS, FIXED_UNVERIFIED, VERIFIED,
BLOCKED, WONT_FIX, REGRESSED.

## 2026-10-30 retrieval-integrity continuation

These local repairs are verified on the candidate binary recorded in
[the checkpoint](RETRIEVAL_INTEGRITY_20261030.md); they are committed on
`fix/retrieval-integrity`. Older entries below retain their historical state.

| ID | Priority | Status | Defect and verification |
|---|---|---|---|
| FX-20261030-002 | P2 | VERIFIED | Context JSON discarded the existing ranked-file list. Rust/Python regressions pass; 65-task result and its one rank loss are recorded |
| FX-20261030-003 | P1 | VERIFIED | Semantic lookup hid stale/truncated evidence and lost live sites behind stale prefixes. Native paired oracle fails before and passes after; identity/position regressions pass |
| FX-20261030-004 | P2 | VERIFIED | Excluded build storms filled the watcher queue before filtering. 16,384-event regression preserves the source edit; rescan/error/Git/overflow controls pass |
| FX-20261030-005 | P3 | VERIFIED | Hardening fixtures inherited the parent Git root with workspace-local TEMP. Explicit fixture Git roots restore all eight native hardening checks |
| FX-20261030-006 | P1 | VERIFIED | A problem report containing an identifier (`MissingSchema error raised when ...`) was answered by the identifier lookup alone: definitions only, empty `ranked`, gold file absent in 4 of 65 dev tasks. Rust regressions (report vs bare lookup vs "where defined") fail before and pass after; paired runs on 120 tasks in FX-BM-020 |
| FX-20261030-008 | P1 | VERIFIED | The plugin launcher exited 3221225781 (0xC0000135, STATUS_DLL_NOT_FOUND) on a machine without the build toolchain: the `findex.exe` built with the gnullvm toolchain imports `libunwind.dll`, which only the build machine has. The MSVC build with the static CRT from `.cargo/config.toml` imports only `bcryptprimitives`, `kernel32`, `ntdll`, `userenv` and API sets. `plugin_check.py` now rejects any runtime importing a DLL absent from System32 (`pe_imports.py`, three tests). Evidence on the MSVC binary: plugin check PASS; Codex app-server loaded the plugin installed by `install-codex.ps1` into an isolated home, listed 12 tools and returned fixture source; three live Haiku sessions through Claude Code (MCP config, `--plugin-dir`, `findex claude`) called `findex_find_symbol` and answered correctly. See [results](../../validation/results/20261030-plugin-portable). No signed or published release exists |
| FX-20261030-007 | P1 | CONFIRMED | Localization targets unmet on a fresh holdout: hit@1 0.345 (target 0.60), gold recall@10 0.761 (target 0.90; the exposed 65 reach exactly 0.900). The exposed 65 overstated generalization by about 20 points. Title-only lexical signals tested on dev (coverage, proximity, weights) do not separate hub files from the small file that holds the fix; candidate recall is not the limit |

## FX-20261029-001 — Retrieval accuracy below target

- Priority: P1
- Status: IN_PROGRESS
- First observed: 2026-10-29 (historical measurement on commit 501bbae baseline)
- Last verified: 2026-10-29 (commit 3dfc311)
- Affected commit: 3dfc311 (HEAD)
- Affected module: crates/findex-core/src/rank.rs, crates/findex-index/src/rank.rs
- Symptom: hit@1 ~44.6% on 65 reused historical issues; gold recall@10 ~80.8%.
- Reproduction steps: Run validation retrieval harness against golden dataset; compare hit@1/recall@10.
- Expected behavior: hit@1 >= 60%, recall@10 >= 90% (roadmap acceptance criteria).
- Actual behavior: hit@1 ~44.6%, recall@10 ~80.8%; targets unmet.
- Root cause (partial, confirmed):
  1. `bm25_score.max(0.0)` in crates/findex-core/src/rank.rs:114 clamped all
     negative FTS5 BM25 scores to 0, collapsing distinct lexical relevance into
     identical bm25_norm = 1.0 (confirmed via reproduction test).
  2. `returned = hits.len()` set before `hits.truncate(limit)` in
     crates/findex-index/src/rank.rs:145, overstating result count
     (confirmed via code inspection).
- Impact: BM25 signal lost for all lexical matches; result count contract
  violated for MCP/CLI consumers.
- Proposed fix: Normalize BM25 as (-score)/(1+(-score)) preserving ordering;
  report count after truncation. Both fixed in commit 3dfc311.
- Regression test: `bm25_negative_scores_produce_distinct_norms` in
  crates/findex-core/src/rank.rs; updated `zero_query_terms_yields_low_score`.
- Resolution: PARTIAL (bugs fixed; accuracy not yet re-measured on HEAD)
- Verification: 220 tests pass; clippy -D warnings clean; fmt clean; release
  build passes. Accuracy re-measurement pending (FX-BM-005).
- Relevant commits: 3dfc311 (fix), 501bbae (baseline measurement)
- GitHub links: https://github.com/brutalstein/findex/commit/3dfc311
- Remaining risks: Ranking changes may regress individual queries; need
  before/after retrieval measurements on golden dataset; candidate generation
  recall not yet audited (Part 5.6).

## FX-20261029-002 — Live model evaluation blocked

- Priority: P1
- Status: BLOCKED
- First observed: 2026-10-29
- Last verified: 2026-10-29
- Affected commit: 0490018 (HEAD)
- Affected module: validation/harness/eval_agents.py, validation/scripts/run-*.ps1
- Symptom: M9 multi-agent live evaluation cannot run.
- Reproduction steps: Attempt `run-all.ps1 -EnableLiveModelTests` without approval/env.
- Expected behavior: Live agent comparison (Claude Code / Codex) with and without Findex.
- Actual behavior: Live tests require `-EnableLiveModelTests`, `$env:FINDEX_LIVE_MODEL_APPROVAL`, non-CI, no API-key vars, and `claude auth status` subscription sign-in.
- Root cause: No authorized model access available in this environment; live tests off by default per validation/README.md.
- Impact: M9 cannot complete; real agent task success data unavailable.
- Proposed fix: Obtain authorized model access; record live results factually (successes and failures).
- Regression test: N/A (live evaluation)
- Resolution: OPEN
- Verification: None
- Relevant commits: 501bbae (eval harness added)
- GitHub links: validation/README.md (Live model tests section)
- Remaining risks: Without live data, claims of agent effectiveness remain inference-based, not measured.

## FX-20261029-003 — Cross-platform CI not yet verified on push

- Priority: P2
- Status: CONFIRMED
- First observed: 2026-10-29 (CI matrix added, commit f3bdd2d)
- Last verified: 2026-10-29 (local clippy/test/build pass on Windows)
- Affected commit: f3bdd2d → 0490018
- Affected module: .github/workflows/ci.yml
- Symptom: CI matrix includes Linux/macOS but has not executed after the push.
- Reproduction steps: Push to main; check GitHub Actions runs.
- Expected behavior: CI passes on windows-latest, ubuntu-latest, macos-latest.
- Actual behavior: CI_NOT_RUN for cross-platform matrix.
- Root cause: Last push (0490018) may not have triggered/finished CI; or CI results not yet recorded.
- Impact: Platform support claimed but not externally verified.
- Proposed fix: Wait for/trigger CI run; record result in STATUS.md.
- Regression test: CI workflow itself.
- Resolution: OPEN
- Verification: Local Windows tests pass; Linux/macOS untested in CI.
- Relevant commits: f3bdd2d (cross-platform CI), 0490018 (CLI tests)
- GitHub links: .github/workflows/ci.yml
- Remaining limitations: Unix-specific path behavior in `storage_dir()` not yet CI-validated.

## FX-20261030-001 — Fresh index cannot be created from the Claude desktop agent sandbox outside the repository tree

- Priority: P3
- Status: SUSPECTED (environment-specific; root cause unconfirmed)
- First observed: 2026-10-30
- Affected commit: e025889 (release and debug binaries)
- Affected module: index creation (`findex-index` `connect`/`synchronize`), run by the Claude desktop app's Bash and PowerShell tools
- Symptom: `findex find-symbol` / `findex index` print `Erişim engellendi. (os error 5)` or `unable to open database file` when `LOCALAPPDATA` points at a new directory outside the repository (for example `C:\Users\cenke\findex-validation-home\...`). Python `sqlite3` (WAL) and the shell can create files in that same directory.
- Reproduction steps: from the agent sandbox, `LOCALAPPDATA=<new dir outside the repo> findex.exe find-symbol <name>` in any project.
- Expected behavior: a fresh index is created under any writable `LOCALAPPDATA`.
- Actual behavior: fails deterministically outside the repository tree; succeeds with state under the repository's `target/` (also tested: debug and release binaries, TEMP inside and outside the tree). Not caused by the `Everyone:(DENY)(DC)` ACE on the repository tree (SQLite opens and writes the database inside that tree; the ACE was left unchanged).
- Impact: none observed for normal interactive use (the real `%LOCALAPPDATA%` holds thousands of index directories). It broke earlier live studies run from the agent sandbox.
- Mitigation: `validation/harness/tool_comparison_live.py` keeps Findex state inside the repository (`FINDEX_LIVE_FINDEX_STATE=target/live-state`) and runs a Findex preflight before spending any model session.
- Remaining limitations: the failing operation was not identified (no syscall trace available in the sandbox).
