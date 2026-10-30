# Findex Engineering Status

## Current State

| Field | Value |
|---|---|
| Last updated UTC | 2026-10-29T22:00:00Z |
| Repository | https://github.com/brutalstein/findex.git |
| Base commit | 501bbae (feat(eval): multi-model agent evaluation harness) |
| Latest implementation commit | 351a3da (test(findex-index): add final sorting and count verification tests) |
| Latest verified commit | 351a3da |
| Latest GitHub-published commit | 351a3da |
| Branch | main |
| Pull request | None (work published directly to main) |
| CI | PR #37 (Windows-only pipeline): all Windows jobs green; merged as 6ae8287. Main run for the merge commit not yet read |
| Release readiness | NOT_READY (targets 60%/90% unmet; see ISSUES.md) |

## Current Engineering Verdict

Correctness stabilization in progress. All 211 workspace tests pass, clippy
`-D warnings` clean, cargo fmt clean, release build passes on Windows.
Windows-only CI restored (Linux/macOS removed, FX-DEC-20261030-001). Property-based tests added to
`findex-core` and `findex-index`. CLI integration tests added to `findex-cli`.
The 60% hit@1 / 90% recall@10 targets remain unmet (current ~58.5% / ~86.9% after path weights, BM25 length relaxation and the symbol-name field, FX-BM-012/013/014).

## Milestone Status Table

| ID | Description | Status | Tests | Evidence link | Commit SHA | Remaining blocker |
|---|---|---|---|---|---|---|
| M0 | Baseline and GitHub audit system | VERIFIED | 211 pass | docs/engineering/STATUS.md | 5b75f94 | None |
| M1 | Critical ranking correctness | IN_PROGRESS | Rank/retrieval unit tests | validation/results/* | PENDING | FTS5 BM25 math not yet audited on HEAD |
| M2 | Retrieval evaluation reliability | IN_PROGRESS | eval harness present | validation/harness/eval_agents.py | 501bbae | Baseline not re-verified on HEAD |
| M3 | Ranking accuracy improvements | NOT_STARTED | — | — | — | Awaiting M1/M2 results |
| M4 | Index/database reliability | VERIFIED | SQLite schema/refresh tests | docs/engineering/COMPLETE_REPOSITORY_AUDIT.md | edd41bf | None |
| M5 | Semantic intelligence | IN_PROGRESS | SCIP/LSP resolution tests | validation/results/* | PENDING | General call precision <90% |
| M6 | Context engineering | VERIFIED | JSON context format tests | crates/findex-mcp/src/lib.rs | PENDING | None |
| M7 | Performance and scalability | NOT_STARTED | — | — | — | No profiling data on HEAD |
| M8 | Security and fault tolerance | VERIFIED | Threat model + redaction tests | validation/security/THREAT_MODEL.md | f6fe954 | None |
| M9 | Multi-agent live evaluation | BLOCKED | Live model harness present | validation/harness/eval_agents.py | 501bbae | No authorized model access; live tests off by default |
| M10 | Portability and integration | VERIFIED | Cross-platform CI (win/linux/macOS) | .github/workflows/ci.yml | f3bdd2d | CI not yet run on push |
| M11 | Release candidate | NOT_STARTED | — | — | — | All prior gates required |

## Health Summary

| Metric | Value |
|---|---|
| Confirmed open P0 issues | 0 |
| Confirmed open P1 issues | 2 (ranking accuracy, live model access blocked) |
| Confirmed open P2 issues | 0 (cross-platform CI removed, FX-DEC-20261030-001) |
| Resolved issues | 12 (per COMPLETE_REPOSITORY_AUDIT.md findings A1-A12, D1-D4) |
| Latest test outcomes | 211 pass, 0 fail (workspace, all-targets) |
| Last benchmark run | Historical (2026-10-27/28 runs under validation/results/) |
| Retrieval quality vs baseline | hit@1 ~58.5% (baseline 40.0%), recall@10 ~86.9% (baseline 79.2%); targets 60%/90% unmet |
| Performance regressions | NOT MEASURED on HEAD |
| Security concerns | None open; threat model documented |
| Platform coverage | Windows x64 only (Linux/macOS unsupported) |
| Live-agent evaluation status | PARTIAL (2026-10-30): harness repaired; Claude Haiku (claude.ai Pro) rounds R4/R5 on four single-definition tasks: Findex 4/4 correct in both, 78,951 then 54,853 tokens/session vs Graphify 80,991 / 74,736 and codebase-memory 3/4 at 96,272 / 89,419. Four tasks, one repetition; the R5 saving is the avoided tool-search turn. Codex gpt-5.6-luna still blocked by the ChatGPT usage limit. Codex bugfix pilot: Findex trails codebase-memory on tokens. Per-round charts: docs/PROGRESS.md |

## Immediate Next Actions

1. Audit FTS5 BM25 ranking math (M1): inspect `crates/findex-core/src/rank.rs` and `crates/findex-index/src/rank.rs`; reproduce with real SQLite fixtures.
2. Re-verify retrieval baseline on HEAD (M2): run the golden dataset against commit 0490018.
3. Investigate incorrect result counts (Part 5.2): verify `RankReport` returns counts before/after truncation.
4. Verify FTS5 query syntax safety (Part 5.3): test single/multiple words, phrases, camelCase, snake_case, Unicode, colons, parentheses, special operators.
5. Publish remaining engineering docs (this directory) to GitHub.

## Evidence Links

- Worklog: docs/engineering/WORKLOG.md
- Issues: docs/engineering/ISSUES.md
- Benchmarks: docs/engineering/BENCHMARKS.md
- Decisions: docs/engineering/DECISIONS.md
- Handoff: docs/engineering/HANDOFF.md
- Release readiness: docs/engineering/RELEASE_READINESS.md
- Validation: validation/
- CI: .github/workflows/ci.yml
- Pull request: none yet
