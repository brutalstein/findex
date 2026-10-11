# Findex Architecture — Technical Foundation v0.1

## North-star outcomes

1. Higher top-k symbol/file/line retrieval recall on real coding tasks.
2. Better downstream task success at equal or lower *total* token usage.
3. Low CPU/RAM/disk overhead and stable latency at scale.
4. No reliance on unofficial Claude authentication or API interception.
5. Correctness-first degradation: no stale evidence, fabricated confidence or hidden fallback failures.

## Context and architecture boundaries

```text
Windows Terminal / PowerShell
  └─ findex.exe claude [official Claude flags]
       └─ official claude.exe (original interactive stdin/stdout)
            └─ bundled Claude Code plugin (optional)
                 └─ findex.exe mcp  [JSON-RPC stdio]
                      └─ findex-index  [SQLite FTS5 + hashes]
                           └─ local project files
```

No source upload or HTTP listener is performed by Findex. Claude Code itself may transmit evidence returned by MCP to Anthropic as part of a task; respect user consent and enterprise policy.

## Implemented paths (v0.1)

- Project identity: nearest ancestor containing `.git` (folder or linked-worktree file), otherwise supplied directory.
- Local state: `%LOCALAPPDATA%/Findex/indexes/<BLAKE3(canonical-root)>/index.sqlite3`.
- Scan: `ignore` walker, no symlink following, 512 KiB per-file max, 100k files per reconciliation. Binary/invalid UTF-8 excluded.
- Indexed state: SQLite `files` metadata and FTS5 `docs` contents; WAL enabled.
- Update: hash every accepted file, replace changed content, remove absent/disallowed paths in a single transaction. Roll back if the safety cap is exceeded.
- Query: escape lexical terms into a bounded disjunction; FTS5 BM25 sorting; verify current file hash before returning file/line/excerpt.
- MCP: stdio JSON-RPC initialize, tools/list and tools/call; errors are explicit. `findex_search`, `findex_symbols`, `findex_find_symbol` and `findex_impact` are exposed.
- CLI: no fabricated saving metrics. `findex claude` launches original native CLI; `doctor` warns when environment API key is present without reading its value.

## Next subsystems (design, not shipped)

### 1. Index Service

The long-lived MCP process already uses Windows file notifications, targeted updates, bounded queues, and periodic/full-repair scans. Standalone CLI queries and imported graph lookups still reconcile the directory. Next: persistent generation snapshots, Git branch/worktree identity, measured crash recovery, and scheduler budgets for very large workspaces. USN journal remains optional and privilege-dependent.

### 2. Structural Knowledge

Tree-sitter definitions are cached and SCIP/captured-LSP reference evidence is imported with source digests. Captured LSP identities are mergeable. Missing: comprehensive language coverage, live LSP synchronization, precise implementation/call relationships and typed cross-service contracts. Dynamic dispatch and unresolved constructs must remain explicitly uncertain.

### 3. Adaptive Retrieval

Cheap lexical recall first; escalate to structural traversal, graph reranking and optional semantic retrieval only on measured benefit. Query intent, result coverage and resource budgets determine planning. Context assembly optimizes evidence diversity under token limits, emits versioned source anchors, and abstains when confidence is insufficient.

### 4. Project Memory

Store verifiable symbol facts and task-derived evidence separately from unverified hypotheses. Revoke stale facts on source/version changes. Prohibit dependency on chat transcript summaries as primary truth.

### 5. Telemetry and Evaluation

Claude Code's official status line or OpenTelemetry where authorized; distinguish total input/output/cache creation/cache reads from context-window consumption. Use paired A/B benchmark tasks with equivalent models, permissions, seeds where supported, and reference gold files/lines. Report top-k recall, MRR, false positives, task success, latency p50/p95, memory peak and total tokens, including regressions. No percent saved without baseline.

## Failure design and threat boundaries

- An unusable Findex index must not prevent the original Claude client from starting.
- No automatic edits to project code, Claude settings or system credential stores.
- Treat project files and returned snippets as untrusted data for instruction-following purposes.
- Refuse unknown or unbounded MCP operations; limit response output.
- Database corruption, file watcher overflow, branch switching and concurrent sessions demand explicit repair and consistency protocols before production release.
- Startup, credentials, plugin permissions and managed enterprise controls must remain under the official Claude Code runtime.

## Open engineering risks

- Real Claude plugin validation and `--plugin-dir` behavior on all targeted versions.
- Native CLI location differences (`claude.exe` vs shell shims) and Windows terminal/ConPTY variation.
- Cargo lockfile reproducibility, pinned third-party actions and signed artifacts.
- FTS5 query behavior on punctuation-heavy identifiers and languages without whitespace boundaries.
- Reconciliation performance for many small files; seek incremental index and worker budgets.
- Secret-scanning of snippets and handling of source texts larger than current limits.


## Query ranking and bounded transport (2026-10-11)

The lexical database uses SQLite FTS5's hidden `rank` column, which by default
evaluates to the BM25 score but has a documented optimization for sorted
retrieval with LIMIT. Ranking remains lexical, *not* a calibrated probability.

The MCP stdio transport now enforces its 1 MiB per-frame byte limit *while
reading* the stream, not after buffering an arbitrarily large unterminated
line. The parser rejects oversized frames without returning them to Claude.
See <https://www.sqlite.org/fts5.html#sorting_by_auxiliary_function_results>.
