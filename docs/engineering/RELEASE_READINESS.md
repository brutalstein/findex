# Findex Release Readiness

Mark checks as passed only when evidence exists. `NOT MEASURED` or `UNKNOWN`
is used when no data is available; never estimated.

## Release target: 0.2.0 (release candidate)

## Checklist

| Area | Check | Status | Evidence | Commit |
|---|---|---|---|---|
| Correctness | All workspace unit + integration tests pass | PASS | 211 pass, 0 fail | 0490018 |
| Correctness | Clippy `-D warnings` clean | PASS | clippy clean | 0490018 |
| Correctness | `cargo fmt --check` clean | PASS | fmt clean | 0490018 |
| Correctness | Release build passes | PASS | `cargo build --release -p findex` | 0490018 |
| Correctness | Property-based tests (path safety, redaction, FTS safety) | PASS | 4 prop tests (core) + 2 (index) | 84fbfbd |
| Correctness | CLI integration tests | PASS | 9 tests | 0490018 |
| Security | Threat model documented (T1-T8, S1-S4) | PASS | validation/security/THREAT_MODEL.md | f6fe954 |
| Security | Secret redaction expanded (Azure, GCP, Datadog, Stripe, SendGrid, Slack) | PASS | crates/findex-core/src/lib.rs | f6fe954 |
| Security | Evidence text sanitization (escape_evidence_line) | PASS | crates/findex-index/src/context.rs | f6fe954 |
| Security | Path traversal / sensitive path rejection tests | PASS | crates/findex-core tests | PENDING |
| Security | No raw secrets committed | PASS | audit of working tree | 0490018 |
| Regression tests | Every confirmed defect has a regression test | PARTIAL | Some findings from AUDIT have fixes+tests; ranking defects open | 0490018 |
| Performance | Cold/incremental indexing time measured | NOT MEASURED | None | — |
| Performance | Query p50/p95 latency measured | NOT MEASURED | None | — |
| Performance | Memory peak measured | NOT MEASURED | None | — |
| Retrieval quality | hit@1 >= 60% | FAIL | ~44.6% (historical, commit 501bbae) | 501bbae |
| Retrieval quality | recall@10 >= 90% | FAIL | ~80.8% (historical, commit 501bbae) | 501bbae |
| Index recovery | Stale/corrupt index detection | PASS | schema user_version check (schema 4) | edd41bf |
| Index recovery | secure_delete on SQLite/FTS5 | PASS | schema 4 | edd41bf |
| MCP compatibility | JSON-RPC stdio; fail-closed; hash-verified evidence | PASS | crates/findex-mcp/src/lib.rs | PENDING |
| Platform validation | Windows native | PASS | CI + local tests | 0490018 |
| Platform validation | Linux | NOT_RUN | CI matrix added, not yet executed | f3bdd2d |
| Platform validation | macOS | NOT_RUN | CI matrix added, not yet executed | f3bdd2d |
| Documentation | README, SECURITY, FINARCH, architecture, roadmap accurate | PARTIAL | Some docs updated (D1-D4 fixed); engineering audit trail new | 0490018 |
| Documentation | Engineering audit trail (this directory) | PASS | 8 documents created | 0490018 |
| Artifact verification | Release ZIP with SHA-256, SBOM, build provenance | NOT READY | No public release published | — |
| Known limitations | Documented in ISSUES.md + RELEASE_CANDIDATE_HARDENING | PASS | docs/engineering/ | 0490018 |

## Release verdict

**NOT READY.**

Blocking items: retrieval accuracy below 60%/90% targets (FX-20261029-001);
cross-platform CI not yet verified after push (FX-20261029-003); performance
not measured (FX-BM-005/006/007); no public release artifact.

## Known limitations

- Accuracy targets unmet on historical baseline; not re-verified on HEAD.
- Live agent evaluation blocked (no authorized model access).
- Linux/macOS support claimed in CI matrix but not yet CI-validated.
- Binaries not Authenticode-signed (SECURITY.md known limitation).
