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
- MCP: stdio JSON-RPC initialize, tools/list and tools/call; errors are explicit. Only `findex_search` exists in v0.1.
- CLI: no fabricated saving metrics. `findex claude` launches original native CLI; `doctor` warns when environment API key is present without reading its value.

## Next subsystems (design, not shipped)

### 1. Index Service

Move from per-query full directory reconciliation to incremental snapshots with Windows file notifications, debouncing, missed-event detection and repair scans. Preserve read-consistent generations, Git branch/worktree identities and crash recovery. USN journal is optional and privilege-dependent. Writes must not block Claude interaction.

### 2. Structural Knowledge

Pluggable Tree-sitter language grammars and LSP/SCIP adapters. Symbol kinds, stable spans, imports, references, calls, runtime uncertainties and provenance. A typed evidence graph relates symbols, files, API endpoints, migrations, schemas and tests. Dynamic dispatch and language-specific constructs are marked uncertain rather than presumed exact.

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
