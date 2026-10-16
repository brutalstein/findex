# Release checklist

Statuses: PASS / FAIL / BLOCKED / NOT_RUN / UNVERIFIED. No aggregate score.

| # | Item | Status | Evidence |
|---|---|---|---|
| 1 | `cargo fmt --all -- --check` | PASS | local, HEAD of `enterprise/p0-safety` |
| 2 | `cargo clippy --workspace --all-targets -- -D warnings` | PASS | local |
| 3 | `cargo test --workspace --all-targets` | PASS | local, native Windows 11 |
| 4 | `cargo build --release -p findex` | PASS | local |
| 5 | CI (Windows stable + MSRV 1.85, cargo audit, SBOM, live-gate spawn test) on the branch | UNVERIFIED until the pushed run is green | GitHub Actions |
| 6 | Offline validation never starts a model client | PASS | `test_live_gate.py` (11) |
| 7 | Coverage claim vs. `git grep`, independent checker, 279 names | PASS (279/279) | CONTEXT_ENGINE_RESULTS |
| 8 | Symbol retrieval regression (642 queries) | PASS (unchanged) | RETRIEVAL_RESULTS |
| 9 | Issue localization regression (65 issues) | PASS (no task worse) | CONTEXT_ENGINE_RESULTS |
| 10 | Black-box security suite | PASS (30/30) | SECURITY_RESULTS |
| 11 | Schema migration 3 → 4, newer schema refused | PASS | MIGRATION_RESULTS |
| 12 | Performance regression (cold index, query, watcher) | PASS (within noise) | PERFORMANCE_RESULTS |
| 13 | Typed semantic graph (P1) | NOT_RUN | SEMANTIC_GRAPH_RESULTS |
| 14 | Live model smoke study (≤ 12 sessions) | NOT_RUN (needs operator approval) | MODEL_VALIDATION |
| 15 | Authenticode signing | BLOCKED (no certificate; none purchased) | — |
| 16 | Official GitHub Release | NOT_RUN (needs operator approval) | — |
| 17 | Windows 10, network volumes, cloud-only OneDrive files, 8.3 names | NOT_RUN | WINDOWS_RELIABILITY |
| 18 | MCP error texts without absolute paths (A11) | FAIL (open) | audit |
| 19 | Index directory ACL test (A12) | NOT_RUN | THREAT_MODEL |
| 20 | Model behaviour under prompt injection | UNVERIFIED | SECURITY_RESULTS |
