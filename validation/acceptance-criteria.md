# Acceptance criteria

A dimension passes only when its criteria were measured in a committed run
directory. `SKIPPED`, `BLOCKED`, `NOT_RUN` and "not measured" never count as
passing. Targets marked *stretch* are reported but do not gate a release.

| # | Dimension | Gate criteria | Evidence |
|---|---|---|---|
| 1 | Correctness | `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`, release build all pass; no known reproducible failure in index/search/symbol/impact/MCP workflows | `tests.json` |
| 2 | Retrieval quality | On the golden set, current Findex is not worse than the baseline build on any overall quality metric; heldout split reported separately. *Stretch:* file Recall@10 >= 0.95, MRR >= 0.90 on `symbol_exact` | `retrieval.json` |
| 3 | Semantic relationship precision | Every direct `findex_impact` edge is verifiable on disk (reference and definition lines name the symbol, aliases resolved); indirect edges are labelled potential, never proven | `relations.json` |
| 4 | Performance | Warm `findex_search` p95 not more than 10 % worse than baseline on every measured corpus; cold-index regressions are reported with their cause | `performance.json`, `regressions.json` |
| 5 | Resource efficiency | Idle CPU of a running MCP server ~0; peak working set and DB size reported per corpus; no unexplained > 10 % regression | `memory.json`, `cpu.json` |
| 6 | Reliability | Concurrent writers, killed indexer, corrupt DB, real watcher edits/renames/deletes all recover without stale or silently truncated results | `tests.json` (`windows_fs`), `security.json`, `performance.json` |
| 7 | Security | No FAIL in `security.json`; `cargo audit` reports no vulnerability; no sockets opened; secrets redacted; path escapes rejected | `security.json`, `tests.json` |
| 8 | Real Claude Code compatibility | Official `claude` connects, discovers tools, performs a real `tools/call` and answers correctly via `--mcp-config`, `--plugin-dir` and `findex claude` | `compatibility.json` |
| 9 | Measured token efficiency | Controlled A/B with >= 3 repetitions per task executed and reported with variance; no claim of savings beyond what the data shows. *Stretch:* lower total tokens with equal or better task success | `tokens.json`, `comparisons.json` |
| 10 | Maintainability and reproducibility | One-command reproduction (`run-all.ps1`), pinned corpora verified, environment and commands recorded, checksums written, docs match the implementation | `manifest.json`, `environment.json`, `commands.jsonl`, `checksums.sha256` |

Mandatory release gates (from the engineering brief): all required format,
lint, build and regression tests pass; no known unresolved critical/high
security defect; no known reproducible correctness failure in supported core
workflows; no misleading stale or silently truncated evidence; real MCP
integration passes on the documented client; supported Windows workflows pass
on the tested machine; performance and token claims are backed by the runs
above; no measured regression is concealed; documentation matches the code.
