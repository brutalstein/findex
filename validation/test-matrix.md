# Test matrix

| Risk | Test | Kind | Evidence |
|---|---|---|---|
| Definition ranked below usages; snake/camel queries miss | `findex-index` unit tests (`definition_outranks_usages…`, `word_query_finds_snake_case…`, `camel_case_identifiers…`, `same_line_co_occurrence…`) | unit | `tests.json` |
| Retrieval quality vs grep and the previous build | `harness/retrieval.py` on 642 golden queries, dev/heldout | benchmark | `retrieval.json` |
| Tree-sitter false definitions (C++ forward decls, Python methods, TS fields, JSX) | `findex-syntax` unit tests | unit | `tests.json` |
| Semantic edge claimed but wrong | `harness/relations.py` (rust-analyzer SCIP, ripgrep) | integration | `relations.json` |
| Unicode, >260-char paths, case-insensitive roots, junctions, locked files, ADS/UNC/device paths, concurrent writers | `crates/findex-index/tests/windows_fs.rs` | Windows integration | `tests.json` |
| Corrupt or legacy index database | `corrupt_index_database_is_rebuilt…`, `legacy_lexical_schema_is_migrated…`; `security.py` random-bytes DB | unit + black-box | `tests.json`, `security.json` |
| Killed indexer leaves a broken DB | `security.py` kill after 0.2/0.6/1.5 s + `PRAGMA integrity_check` | black-box | `security.json` |
| Watcher misses real edits/creates/renames/deletes | `perf.py` edit workload on a ripgrep copy | black-box | `performance.json` |
| Malformed MCP frames crash or wedge the server | `findex-mcp` unit tests; `security.py` 2 MiB line, invalid JSON, 5000-deep nesting, batch | unit + black-box | `tests.json`, `security.json` |
| MCP protocol era mismatch (legacy `initialize` vs 2026-07-28 stateless `_meta`) | `findex-mcp` unit tests; `security.py` discover / -32022; trace of real Claude Code | unit + integration | `tests.json`, `security.json`, `compatibility.json` |
| Secrets leak through excerpts; `.env` indexed | `redaction_masks…`, `excerpts_redact…`; `security.py` | unit + black-box | `tests.json`, `security.json` |
| Prompt injection inside indexed source | MCP `instructions` mark results untrusted; `security.py` returns injection text only as JSON data | unit + black-box | `tests.json`, `security.json` |
| Network access | `security.py` socket sampling while indexing and serving | black-box | `security.json` |
| Vulnerable dependency | `cargo audit` | supply chain | `tests.json` |
| Plugin/launcher packaging broken | `claude_integration.py` doctor + `--plugin-dir` + `findex claude` | real client | `compatibility.json` |
| Token savings claimed without evidence | `claude_ab.py` 6 tasks × 2 arms × 3 reps | real client | `tokens.json`, `comparisons.json` |
| Performance/resource regression | `perf.py` baseline vs current on ripgrep, tokio, vscode | benchmark | `performance.json`, `memory.json`, `cpu.json`, `regressions.json` |
