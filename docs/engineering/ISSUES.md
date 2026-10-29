# Findex Engineering Defect Registry

Priorities: P0 = critical security/data integrity/catastrophic functional;
P1 = important functionality or accuracy broken; P2 = significant
performance/reliability/maintainability; P3 = lower impact or documentation.

Statuses: SUSPECTED, CONFIRMED, IN_PROGRESS, FIXED_UNVERIFIED, VERIFIED,
BLOCKED, WONT_FIX, REGRESSED.

## FX-20261029-001 — Retrieval accuracy below target

- Priority: P1
- Status: CONFIRMED
- First observed: 2026-10-29 (historical measurement on commit 501bbae baseline)
- Last verified: 2026-10-29 (docs/roadmap.md context locality checkpoint)
- Affected commit: 0490018 (HEAD)
- Affected module: crates/findex-index (ranking), crates/findex-core (rank model)
- Symptom: hit@1 ~44.6% on 65 reused historical issues; gold recall@10 ~80.8%.
- Reproduction steps: Run validation retrieval harness against golden dataset; compare hit@1/recall@10.
- Expected behavior: hit@1 >= 60%, recall@10 >= 90% (roadmap acceptance criteria).
- Actual behavior: hit@1 ~44.6%, recall@10 ~80.8%; targets unmet.
- Root cause: Not yet isolated; candidate causes include BM25 transform, candidate generation window, feature weighting.
- Impact: Findex does not meet self-declared accuracy targets; engineering task completion may suffer.
- Proposed fix: Audit FTS5 BM25 math (Part 5.1), verify result counts (Part 5.2), FTS5 query syntax (Part 5.3), final sorting (Part 5.4), feature ablation (Part 5.5), candidate recall (Part 5.6).
- Regression test: Golden dataset run before/after each ranking change.
- Resolution: OPEN
- Verification: None yet
- Relevant commits: 501bbae (baseline measurement), 0490018 (HEAD)
- GitHub links: docs/engineering/STATUS.md (milestone M1)
- Remaining risks: Ranking changes may regress individual queries even if aggregate improves.

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
