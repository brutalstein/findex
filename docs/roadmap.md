# Findex Roadmap and Acceptance Criteria

Status means *evidence*, not aspiration. No milestone is considered complete merely because source files exist.

## Quality targets checkpoint (2026-10-28)

- [x] Bounded concept retrieval and local Rust receiver-annotation evidence;
  explicit source/scope on every context report.
- [x] Native stable/MSRV: 167 Rust tests; 42 offline Python tests, 29 security
  checks, 115 unchanged navigation hit lists, 199 coverage claims and 960 budget
  checks pass.
- [ ] Real issue hit@1 >=60% and gold recall@10 >=90%: current 40.0% and 79.2%.
- [ ] General call-resolution quality >=90%: conditional macro precision exceeds
  90%, but Tokio micro precision is 88.1% and confident coverage falls sharply.
- [ ] Same-model/budget bugfix improvement: the new live campaign was blocked by
  execution permissions; no evaluable bugfix trials. The user declined new starts.
- [ ] Repackage and revalidate the new engine for a public release. Earlier
  package/cloud/SMB/scale evidence remains tied to the previous binary.

[Measured results and limitations](engineering/QUALITY_TARGETS_20261028.md).

## Release-candidate hardening (2026-10-27)

- [x] Lossless compact MCP search/symbol output; 115 roundtrips and unchanged
  source-hit lists against the previous binary, including 55 new queries.
- [x] Redaction-before-truncation regression and explicit bounded-symbol
  truncation diagnostics.
- [x] Native installed Codex plugin: eight tools, fixture source and compact
  output; six real Codex subscription sessions, all answers correct (small pilot).
- [x] Evidence guards for frozen sources/tools, interrupted live attempts,
  successful MCP calls and nDCG duplicate ranks; offline regressions in CI.
- [x] Bounded native I/O and metadata-first full-text retrieval, with unchanged
  reference results and concurrent-read-snapshot regressions on stable/MSRV.
- [x] Synthetic 200–100k-file native scaling, bounded idle resources and
  100001-file transactional rollback/recovery; arbitrary sizes remain unproven.
- [x] Codex upgrades while the previous executable is locked, repeat installation
  and corrupt-runtime refusal; verified immutable runtimes preserve active sessions.
- [ ] Universal superiority over alternatives: not demonstrated. Codebase-memory
  tree output remains smaller on three corpora and historical controls; code-edit,
  architecture and large live studies
  are outside the new pilot. See `docs/EVIDENCE.md` and the dated release report.

The initial milestone checklist below is historical planning, supplemented by
the dated evidence sections; unchecked future goals are not release promises.

## F0 — Foundation (validated Windows CI)

- [x] Four-crate Rust workspace, separate CLI/core/index/MCP responsibilities.
- [x] Source for Windows native official Claude CLI forwarding and optional plugin.
- [x] Source for local SQLite/FTS5 index with transactional updates and stale-hash filtering.
- [x] Source tests for file edit/removal, secret filename exclusions, MCP calls and project roots.
- [x] Windows build/test/package workflow checked into source tree.
- [x] Compile and unit-test on Windows CI (stable and Rust 1.85 MSRV).
- [ ] Confirm Claude Code plugin validation and actual MCP tool invocation.
- [ ] Publish first signed or explicitly unsigned preview artifact on GitHub.

## F1 — CI + Reliable Bootstrap

- [x] Include and enforce Cargo.lock in CI; vulnerability and license audit still pending.
- [x] CI `cargo fmt`, `clippy`, tests and scripted MCP smoke; real Claude launcher integration still pending.
- [ ] Launch/system tests on Windows 10/11; policy-compatible PATH and legacy shell-shim handling.
- [ ] CI branch protection, dependency updates, safe release provenance.
- [ ] End-to-end plugin integration checked against documented Claude Code versions.

## F2 — Change-Aware Index

- [x] Native Windows watcher and full-rescan recovery; branch/worktree stress testing remains pending.
- [ ] Reconciliation over 100k files without blocking interactive queries.
- [ ] Atomic generation-based index snapshots and multi-session concurrency tests.
- [ ] Path traversal, symlink/junction, OneDrive/network volume and Unicode case-fold tests.

## F3 — Structural Graph

- [x] Experimental Tree-sitter adapter and bounded definition spans for four language families; capability coverage is not complete.
- [x] Bounded captured LSP and SCIP import with source-hash verification; live LSP lifecycle is not implemented.
- [ ] Cross-language and service contract graph; captured-LSP symbol merges and source invalidation are partial building blocks.
- [ ] Real-data recall@k / line recall comparisons against lexical baseline.

## F4 — Adaptive Context and Validation

- [ ] Query planner, graph reranker and optional semantic embedding runner.
- [ ] Context-budget optimizer with citation/evidence anchors and graceful abstention.
- [ ] Verified memory lifecycle and deterministic source-impact reporting.
- [ ] Code-fix A/B outcomes measured against standard Claude Code and existing alternatives.

## F5 — Global Product Quality

- [ ] Opt-in telemetry with privacy controls, English first-party docs and full accessibility.
- [ ] Resource budgets per machine and reproducible benchmark reference suite.
- [ ] Red-team prompt-injection and secret-leak exercises; crash/recovery soak tests.
- [ ] Secure update, signed binary, SBOM, coordinated vulnerability disclosure and enterprise policy support.

## Evidence added after initial roadmap

- [x] Native Windows watcher and transactional known-file refresh; CLI cold
  searches and new paths still require reconciliation.
- [x] On-demand four-language Tree-sitter definition extraction in CLI and MCP.
- [x] Revision- and digest-keyed SQLite persistent symbol definitions (first
  slice, exact-name lookup, parser cache; complete after new CI verification).
- [ ] Native watcher-backed persistent symbol lookups without full project walk.
- [ ] Multi-session consistency, branch awareness, cross-language semantic
  reference graphs, benchmarks and signed production release.

- [x] Native watcher-backed exact-symbol MCP retrieval with a warm-query
  no-full-scan CI assertion, plus indexed-only AST access controls.

## Semantic evidence graph milestone (2026-10-11)

- [x] SCIP protobuf and normalized captured LSP reference adapters, with file
  admission, source hash, budget and transactional safety controls.
- [x] Reverse file-impact BFS on exact semantic symbol IDs with bounded depth,
  stale invalidation, cycle handling, source/line provenance and MCP/CLI tools.
- [ ] Live language server orchestration and document version synchronization (captured LSP UTF-16 position bounds are now checked).
- [ ] SCIP streaming import for very large generated indexes.
- [ ] Precise type/implementation/call graphs and cross-indexer federation.
- [ ] Independent real-world correctness and performance benchmarks.

## Validation milestone (2026-10-12)

Evidence: `validation/FINAL_VALIDATION_REPORT.md` and `validation/results/`.

- [x] Real Claude Code plugin validation and MCP tool invocation (`--mcp-config`,
  `--plugin-dir`, `findex claude`) on Claude Code 2.1.281 — closes the F0/F1
  integration items for that version.
- [x] Real-data recall@k / line recall vs a lexical (`git grep`) baseline and
  the previous build on 642 golden queries over 7 pinned corpora (F3).
- [x] Path traversal, junction, Unicode, long-path, case-fold, locked-file and
  concurrent-writer tests on Windows (F2 partial; OneDrive/network volumes
  still untested).
- [x] Cold indexing no longer O(files²): FTS rows keyed by file id.
- [x] RustSec audit job, CycloneDX SBOM in the release ZIP, build-provenance
  attestation on non-PR CI runs (F5 partial; binaries still unsigned).
- [ ] Code-fix A/B outcomes: a small Haiku study exists; larger models, more
  tasks and comparison with other navigation tools remain (F4).
- [ ] Query planner beyond definition-first + lexical, graph reranking,
  embeddings (F4) — not justified by measured failures yet.

## Call-site precision milestone (2026-10-26)

Evidence: `docs/EVIDENCE.md`, `validation/results/20261026-*`.

- [x] Owner-qualified caller precision on the frozen SCIP held-out split:
  ripgrep 0.523 → 0.714, tokio 0.376 → 0.567, with site recall up
  (0.957 → 0.981, 0.829 → 0.947). Known false negatives listed in EVIDENCE.
- [x] Coverage line still 279/279 against `git grep -w`.
- [x] Router: new frozen held-out v2, 57/60.
- [x] Live tests bill only the subscription (per-session `apiKeySource` check).
- [ ] Typed multi-language graph, Authenticode, Windows 10/SMB/OneDrive tests,
  token results beyond Haiku navigation tasks.
