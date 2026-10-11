# Findex Roadmap and Acceptance Criteria

Status means *evidence*, not aspiration. No milestone is considered complete merely because source files exist.

## F0 — Foundation (initial source tree; pending CI)

- [x] Four-crate Rust workspace, separate CLI/core/index/MCP responsibilities.
- [x] Source for Windows native official Claude CLI forwarding and optional plugin.
- [x] Source for local SQLite/FTS5 index with transactional updates and stale-hash filtering.
- [x] Source tests for file edit/removal, secret filename exclusions, MCP calls and project roots.
- [x] Windows build/test/package workflow checked into source tree.
- [ ] Compile and unit-test on Windows CI (not yet executed).
- [ ] Confirm Claude Code plugin validation and actual MCP tool invocation.
- [ ] Publish first signed or explicitly unsigned preview artifact on GitHub.

## F1 — CI + Reliable Bootstrap

- [ ] Include and pin Cargo.lock; dependency license and vulnerability scanning.
- [ ] CI `cargo fmt`, `clippy`, tests, manual MCP smoke, launcher process passthrough smoke.
- [ ] Launch/system tests on Windows 10/11; policy-compatible PATH and legacy shell-shim handling.
- [ ] CI branch protection, dependency updates, safe release provenance.
- [ ] End-to-end plugin integration checked against documented Claude Code versions.

## F2 — Change-Aware Index

- [ ] File watcher + rescan recovery; branch and worktree awareness.
- [ ] Reconciliation over 100k files without blocking interactive queries.
- [ ] Atomic generation-based index snapshots and multi-session concurrency tests.
- [ ] Path traversal, symlink/junction, OneDrive/network volume and Unicode case-fold tests.

## F3 — Structural Graph

- [ ] Tree-sitter adapter and source-span extraction, language capability registry.
- [ ] LSP/SCIP adapters with explicit precision/provenance and timeout budgets.
- [ ] Cross-language and service contract graph with incremental invalidation.
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
