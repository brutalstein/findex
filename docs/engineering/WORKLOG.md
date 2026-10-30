# Findex Engineering Worklog

Append-only chronological record of substantive engineering events. Historical
entries are never deleted or rewritten; follow-up entries use new IDs.

Format: `## [UTC timestamp] [Event ID] Short description`

## [2026-10-29T22:00:00Z] [FX-20261029-001] Establish engineering audit trail

- Branch: main
- Starting commit: 0490018
- Current implementation commit: 0490018
- Subsystem: docs/engineering
- Issue ID: Part 2 mandatory structure
- Objective: Create permanent GitHub-visible engineering audit trail (README, STATUS, WORKLOG, ISSUES, BENCHMARKS, DECISIONS, HANDOFF, RELEASE_READINESS) and link STATUS.md from root README.md.
- Files inspected: AGENTS.md, README.md, SECURITY.md, FINARCH.md, docs/roadmap.md, docs/engineering/*.md, validation/README.md, .github/workflows/ci.yml
- Files modified: docs/engineering/README.md, docs/engineering/STATUS.md, docs/engineering/WORKLOG.md, docs/engineering/ISSUES.md, docs/engineering/BENCHMARKS.md, docs/engineering/DECISIONS.md, docs/engineering/HANDOFF.md, docs/engineering/RELEASE_READINESS.md, README.md
- Reproduction command: N/A (documentation only)
- Observed behavior: No permanent engineering audit structure existed beyond ad-hoc docs/engineering/*.md files.
- Expected behavior: A discoverable, version-controlled engineering record per Part 2 of the engineering mandate.
- Error or failure: None
- Root cause: N/A
- Fix implemented: Created all eight required documents and added STATUS.md link to root README.md.
- Tests executed: None (documentation only; will run cargo fmt/clippy/test after commit)
- Test outcomes: N/A
- Benchmark impact: None
- Evidence location: docs/engineering/
- Commit SHA: 5b75f94
- GitHub URL: https://github.com/brutalstein/findex/commit/5b75f94
- Remaining limitations: CI has not yet run on the cross-platform matrix after the last push.
- Next action: Begin M1 ranking correctness audit (FTS5 BM25 math).

## [2026-10-29T22:30:00Z] [FX-20261029-005] M1: BM25 score collapse + result count bugs fixed

- Branch: main
- Starting commit: e4d40f9
- Current implementation commit: 3dfc311
- Subsystem: crates/findex-core/src/rank.rs, crates/findex-index/src/rank.rs
- Issue ID: FX-20261029-001 (retrieval accuracy below target)
- Objective: Fix confirmed ranking correctness defects identified in M1 audit.
- Files inspected: crates/findex-core/src/rank.rs (rank_file BM25 normalization),
  crates/findex-index/src/rank.rs (rank_in_db result count)
- Files modified: crates/findex-core/src/rank.rs, crates/findex-index/src/rank.rs
- Reproduction command: cargo test --package findex-core rank::tests::bm25_negative_scores_produce_distinct_norms
- Observed behavior: FTS5 BM25 returns negative values (more negative = better).
  `bm25_score.max(0.0)` clamped all negatives to 0, collapsing distinct lexical
  relevance into identical bm25_norm = 1.0. `returned = hits.len()` was set
  before `hits.truncate(limit)`, overstating result count.
- Expected behavior: BM25 score ordering preserved; returned count reflects
  hits after truncation.
- Error or failure: Two tests initially failed after fix (bm25_negative test
  asserted old buggy behavior; zero_query test expected norm 1.0 for score 0.0).
- Root cause: Incorrect sign handling on FTS5 negative BM25 scores; count
  reported before truncation.
- Fix implemented:
  1. BM25 normalized as `(-score).max(0.0) / (1.0 + (-score).max(0.0))`,
     preserving ordering (more negative → higher norm).
  2. `hits.truncate(limit)` before `let returned = hits.len()`.
- Tests executed: cargo test --workspace --all-targets → 220 pass
- Test outcomes: All pass
- Benchmark impact: None yet (accuracy re-measurement pending)
- Evidence location: crates/findex-core/src/rank.rs, crates/findex-index/src/rank.rs
- Commit SHA: 3dfc311
- GitHub URL: https://github.com/brutalstein/findex/commit/3dfc311
- Remaining limitations: Accuracy not yet re-measured on HEAD; candidate
  generation recall not yet audited; FTS5 query syntax and final sorting
  still to verify under M1.
- Next action: Re-verify retrieval baseline on HEAD (golden dataset), then
  verify FTS5 query syntax safety and final sorting.

## [2026-10-29T23:00:00Z] [FX-20261029-006] M1: FTS5 query syntax safety tests added

- Branch: main
- Starting commit: 5f28972 (before this entry)
- Current implementation commit: 5f28972
- Subsystem: crates/findex-index/src/lib.rs
- Issue ID: FX-20261029-001 (retrieval accuracy below target), Part 5.3
- Objective: Add comprehensive FTS5 query syntax safety tests covering
  single/multiple words, camelCase, snake_case, quoted phrases, colons,
  parentheses, operators, asterisk, empty queries, term cap, and real
  SQLite integration.
- Files inspected: crates/findex-index/src/lib.rs (fts_query, split_query_terms,
  search_in_db)
- Files modified: crates/findex-index/src/lib.rs (+162 lines, 14 new tests)
- Reproduction command: cargo test --package findex-index fts_query
- Observed behavior: Existing tests only covered basic operator neutralization
  and term cap. No coverage for camelCase expansion, snake_case, colons,
  parentheses, operator-as-term, asterisk, empty queries, or real SQLite
  integration with count-respects-limit.
- Expected behavior: FTS5 query construction neutralizes all operators;
  camelCase expands to sub-words; snake_case is single term; empty query
  returns None; term cap at 12; search_in_db count never exceeds limit;
  operator-like input doesn't crash.
- Error or failure: clippy unused variable on `terms` in
  fts_query_quoted_phrase test; fmt line-wrapping on long search_in_db calls.
- Root cause: Unused variable; fmt prefers single-line for short calls.
- Fix implemented: Renamed to `_terms`; cargo fmt auto-fixed line wrapping.
- Tests executed: cargo test --workspace --all-targets → 234 pass
- Test outcomes: All pass
- Benchmark impact: None
- Evidence location: crates/findex-index/src/lib.rs (tests 1618-1740 approx)
- Commit SHA: 5f28972
- GitHub URL: https://github.com/brutalstein/findex/commit/5f28972
- Remaining limitations: Real retrieval accuracy not yet measured on HEAD;
  final sorting verification still pending under M1.
- Next action: Run golden dataset against 5f28972; verify final sorting
  (sort-before-truncate, tie-breaking).

## [2026-10-29T21:00:00Z] [FX-20261029-002] (carried from prior session) CLI integration tests

- Branch: main
- Starting commit: f3bdd2d
- Current implementation commit: 0490018
- Subsystem: crates/findex-cli
- Issue ID: Part 13 test engineering
- Objective: Add end-to-end integration tests for all CLI commands (help, version, unknown command, index, search, find-symbol, context text+JSON, status).
- Files inspected: crates/findex-cli/src/main.rs, crates/findex-cli/Cargo.toml
- Files modified: crates/findex-cli/Cargo.toml (added tempfile dev-dependency), crates/findex-cli/tests/cli_integration.rs (new)
- Reproduction command: cargo test --package findex
- Observed behavior: findex-cli had 0 tests; no coverage of CLI surface.
- Expected behavior: CLI commands verified against real temporary repositories.
- Error or failure: clippy `expect_used` on test helpers; resolved with `#![allow(clippy::expect_used)]` at test file level.
- Root cause: Test helpers use `expect()` for binary path resolution and process spawning.
- Fix implemented: Allowed `expect_used` in test-only file; all 9 tests pass.
- Tests executed: cargo test --package findex → 9 passed
- Test outcomes: 9 pass, 0 fail
- Benchmark impact: None
- Evidence location: crates/findex-cli/tests/cli_integration.rs
- Commit SHA: 0490018
- GitHub URL: https://github.com/brutalstein/findex/commit/0490018
- Remaining limitations: CLI tests require git to be available for project_root; run only where git exists.
- Next action: Continue with M1 ranking correctness audit.

## [2026-10-29T20:30:00Z] [FX-20261029-003] (carried from prior session) Cross-platform CI

- Branch: main
- Starting commit: 84fbfbd
- Current implementation commit: f3bdd2d
- Subsystem: .github/workflows/ci.yml, crates/findex-core/src/lib.rs
- Issue ID: M10 Portability and integration
- Objective: Extend CI from Windows-only to Windows/Linux/macOS matrix; make `storage_dir()` cross-platform.
- Files inspected: .github/workflows/ci.yml, crates/findex-core/src/lib.rs
- Files modified: .github/workflows/ci.yml (matrix: windows-latest, ubuntu-latest, macos-latest), crates/findex-core/src/lib.rs (LOCALAPPDATA on Windows; XDG_CACHE_HOME/HOME on Unix)
- Reproduction command: cargo clippy --workspace --all-targets -- -D warnings; cargo test --workspace --all-targets; cargo build --release -p findex
- Observed behavior: CI ran only on Windows; storage_dir() required LOCALAPPDATA (Windows-only).
- Expected behavior: CI validates all supported platforms; storage directory works on Unix.
- Error or failure: clippy `needless-character-iteration` on `chars().all(|c| c.is_ascii())`; resolved with `bytes()`/`is_ascii()` checks.
- Root cause: clippy lint flags iterator-based ASCII checks.
- Fix implemented: Replaced `chars().all()` with `bytes().all()` and `is_ascii()`; CI matrix added.
- Tests executed: 211 pass; clippy -D warnings clean; cargo fmt clean; release build passes
- Test outcomes: All pass
- Benchmark impact: None
- Evidence location: .github/workflows/ci.yml
- Commit SHA: f3bdd2d
- GitHub URL: https://github.com/brutalstein/findex/commit/f3bdd2d
- Remaining limitations: CI has not yet executed on the pushed cross-platform matrix (CI_NOT_RUN).
- Next action: Verify CI passes on Linux/macOS; then M1 ranking audit.

## [2026-10-29T20:00:00Z] [FX-20261029-004] (carried from prior session) Property-based tests

- Branch: main
- Starting commit: 501bbae
- Current implementation commit: 84fbfbd
- Subsystem: crates/findex-core, crates/findex-index
- Issue ID: M9 PHASE 9-10 Test Architecture
- Objective: Add proptest property-based tests for path safety, secret redaction, FTS query safety, term cap.
- Files inspected: crates/findex-core/src/lib.rs, crates/findex-index/src/lib.rs
- Files modified: crates/findex-core/Cargo.toml (proptest dev-dep), crates/findex-core/src/lib.rs (4 prop tests), crates/findex-index/Cargo.toml (proptest dev-dep), crates/findex-index/src/lib.rs (2 prop tests)
- Reproduction command: cargo test --workspace --all-targets; cargo clippy --workspace --all-targets -- -D warnings
- Observed behavior: proptest `string_regex().unwrap()` triggered clippy `unwrap_used`/`expect_used` denies.
- Expected behavior: Property tests compile clean under `#![deny(clippy::unwrap_used, clippy::expect_used)]`.
- Error or failure: clippy `unwrap_used`/`expect_used`; clippy `needless-character-iteration`; clippy `ok_expect`.
- Root cause: proptest strategies return `Result`; tests used unwrap/expect; iterator ASCII checks flagged.
- Fix implemented: Replaced regex strategies with `any::<String>()` + `prop_assume!()` guards; replaced `ok().expect()` with `match`; replaced `chars().all()` with `bytes().all()`.
- Tests executed: 202 pass (before CLI tests); clippy clean; cargo fmt clean; release build passes
- Test outcomes: All pass
- Benchmark impact: None
- Evidence location: crates/findex-core/src/lib.rs, crates/findex-index/src/lib.rs
- Commit SHA: 84fbfbd
- GitHub URL: https://github.com/brutalstein/findex/commit/84fbfbd
- Remaining limitations: proptest tests use `any::<String>()` with assume guards rather than bounded regex strategies; coverage of adversarial inputs is narrower than originally intended.
- Next action: Complete cross-platform CI, then M1 ranking audit.

## [2026-10-29T23:15:00Z] [FX-20261029-007] M1: Final sorting and count verification tests added

- Branch: main
- Starting commit: 3f63d56
- Current implementation commit: 351a3da
- Subsystem: crates/findex-index/src/rank.rs
- Issue ID: FX-20261029-001 (retrieval accuracy below target), Part 5.4
- Objective: Add tests verifying final sort happens before truncation,
  `returned` count is accurate, scores are non-increasing, and tie-breaking
  is deterministic.
- Files inspected: crates/findex-index/src/rank.rs (rank_in_db sort/truncate)
- Files modified: crates/findex-index/src/rank.rs (+76 lines, 3 new tests)
- Reproduction command: cargo test --package findex-index rank_in_db
- Observed behavior: `rank_in_db` already sorted by final combined score and
  truncated correctly (fixed in 3dfc311), but had no dedicated tests for
  count accuracy, score ordering, or tie-breaking.
- Expected behavior: `returned` == hits.len() after truncation; hits never
  exceed limit; scores non-increasing; equal scores ordered by path ascending.
- Error or failure: None (code was already correct after 3dfc311 fix).
- Root cause: Missing test coverage, not a code defect.
- Fix implemented: Added 3 tests covering count/truncate, score ordering,
  path tie-breaking.
- Tests executed: cargo test --workspace --all-targets → 237 pass
- Test outcomes: All pass
- Benchmark impact: None
- Evidence location: crates/findex-index/src/rank.rs
- Commit SHA: 351a3da
- GitHub URL: https://github.com/brutalstein/findex/commit/351a3da
- Remaining limitations: Real retrieval accuracy not yet measured on HEAD
  (golden dataset run pending); feature quality audit and candidate recall
  audit still pending under M1.
- Next action: Run golden dataset against 351a3da to measure before/after
  accuracy (FX-BM-005).

## 2026-10-30 — retrieval-integrity continuation from 28842c5

- Recovered the actual R12 state; the earlier handoff's 351a3da baseline was
  stale. Used Graphify and Findex context/search/symbol tools for investigation.
- Reproduced and repaired context JSON rank loss, silent semantic
  stale/truncated results, stale-prefix false negatives and pre-filter watcher
  queue overflow. Isolated hardening fixtures from the parent Git root.
- Native Windows fmt, Clippy, 249 Rust tests, release and 63 Python tests pass;
  six real compiler-produced SCIP cases, 115 unchanged MCP navigation controls,
  12 self-hosted context/budget checks and eight hardening checks pass.
- Frozen 65-task localization: hit@1 58.46%, gold recall@10 86.92%, unchanged;
  fmt-2695 first gold rank 3 to 4 reflects explicit JSON rank propagation and
  slightly decreases MRR. No ranking weights were tuned.
- Initial permission/ACL/fixture-root failures are retained, not counted as
  successful runs. Only a new test-state directory's ACL was narrowed.
- Independent bounded review found no introduced actionable defect. Graphify
  refreshed with AST extraction only. A later Codex install and four-start paired bugfix
  pilot is recorded in validation/results/20261030-retrieval-delivery/NOTE.md.
- Evidence and next gates: RETRIEVAL_INTEGRITY_20261030.md and
  validation/results/20261030-retrieval-integrity/.

## 2026-10-30 — fresh holdout, candidate-recall decomposition, report routing

- Froze `issue_localization_v2.jsonl` (55 tasks, disjoint from the 65 exposed; zod exhausted) before scoring.
  Previous binary: hit@1 0.364, recall@10 0.706 against 0.585 / 0.869 on the exposed set.
- `candidate_recall.py` on the 65 dev tasks: 0 gold files outside the 128-candidate window, 2 never matched,
  14 deeper than rank 10. Four of those were missing from the output although raw BM25 had them at rank 2-18;
  all four were identifier-routed reports with an empty `ranked`.
- Shipped one change (d4718e9): a task with an identifier plus at least two descriptive words also runs the
  concept search. TDD: the new Rust regression failed first. Paired runs: fresh recall@10 0.706 to 0.761,
  exposed 0.869 to 0.900; hit@1 0.364 to 0.345 and 0.585 unchanged.
- Rejected on dev, no code kept: term coverage, term proximity, b / filename / symbol / path weight sweeps.
- Harness: `freeze_binary` shared by the localization scripts after `target/release/findex.exe` returned
  access denied outside the checkout (the content-addressed copy under the validation home works);
  `--dataset` for localization and rivals.
- Not done: compiler-backed context adapter, bounded dirty-path scheduling, snapshot generations.

