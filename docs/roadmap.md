# Findex Roadmap and Acceptance Criteria

Status means *evidence*, not aspiration. No milestone is considered complete merely because source files exist.

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
