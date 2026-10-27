# Findex Architecture

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

Codex loads the shared skills and MCP engine through `plugin/.codex-plugin`;
`plugin/scripts/install-codex.ps1` binds the installed native binary. Both
official clients retain control of their own permissions and authentication.
No source upload or HTTP listener is performed by Findex. Claude Code or Codex
may transmit returned evidence to its provider; respect user consent and enterprise policy.

## Implemented paths

- Project identity: nearest ancestor containing `.git` (folder or linked-worktree file), otherwise supplied directory.
- Local state: `%LOCALAPPDATA%/Findex/indexes/<BLAKE3(canonical-root)>/index.sqlite3`.
- Scan: `ignore` walker, no symlink following, 512 KiB per-file max, 100k files per reconciliation. Binary/invalid UTF-8 excluded.
- Indexed state: SQLite `files` metadata, FTS5 `docs(path, body, terms)` where `terms` holds camelCase sub-words and path words, and a Tree-sitter `symbols` table with an expression index on the normalized identifier (`lower(replace(name,'_',''))`); WAL and `secure_delete` enabled, FTS5 `secure-delete` on, so removed text does not stay in free pages or index segments. `PRAGMA user_version` versions the lexical schema; older lexical rows are rebuilt on reconciliation. Corrupt databases are refused and preserved for explicit recovery; newer schemas are refused without mutation.
- Update: hash every accepted file, replace changed content, remove absent/disallowed paths in a single `BEGIN IMMEDIATE` transaction (concurrent writers wait on the busy timeout instead of failing). Roll back if the safety cap is exceeded. A known file whose read fails for a reason other than absence (locked mid-save) keeps its entry until a later reconciliation can read it.
- Query: (1) definitions whose normalized name equals the concatenated query (`GitignoreBuilder`, `gitignore builder`, `gitignore_builder`), types before functions before methods; (2) FTS5 candidates ranked by query-term coverage per file, then same-line co-occurrence, then BM25. Only returned hits are re-verified against the current file hash; stale hits are counted, never returned; `truncated` reports remaining candidates. Excerpts pass through `redact_secrets`.
- MCP: stdio JSON-RPC. Legacy `initialize` (2025-11-25, 2025-06-18, 2025-03-26, 2024-11-05) and the 2026-07-28 stateless era (`server/discover`, per-request `_meta` protocol version, `-32022` for unsupported versions) are both served. Oversized frames, invalid JSON and invalid requests get JSON-RPC errors and the server keeps reading. `findex_context` (marked `anthropic/alwaysLoad`), `findex_search`, `findex_symbols`, `findex_find_symbol` and `findex_impact` are exposed; server `instructions` mark returned source as untrusted data.
- Context engine (`findex-index/src/context.rs`): a rule-based router maps the task text (English and Turkish intent words) to `definition`, `callers`, `tests`, `impact`, `file` or `concept`. Identifier tasks get Tree-sitter definitions plus every word-boundary occurrence in files the FTS index lists, classified as call/reference/import by the following token, with comment lines omitted by a per-language lexer (`//`/`/* */` with nesting for Rust/Swift/Kotlin/Scala, `#`, multi-line and raw strings) and counted. File tasks list the named file's definitions; concept tasks run the lexical query. Evidence is packed greedily under a character/4 token budget; an untruncated report states exhaustive coverage. Call sites are name matches, not type-resolved; relations/SCIP data are not used here.
- Validation: the runner is offline by default; real Claude sessions need `--enable-live-model-tests`, an approval string and subscription checks (`validation/harness/claude_session.py`), and CI asserts that no Claude process is spawned.
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

## Context retrieval and receiver annotations (2026-10-28)

Natural-language defect queries use a context-specific BM25 path rather than
identifier-first lexical ranking. The existing search/symbol MCP ranking stays
unchanged. Context considers at most 128 lexical candidates and returns at most
32 ranked lexical files before rendered-budget packing. Named-symbol site scans
retain their separate 400-file limit. Split snake-case terms help
locate related implementations when a requested identifier does not exist.
Candidate and output caps propagate to `truncated` and `missing`.

Rust method candidates can use local parameter and explicit local-variable type
annotations. The syntax adapter respects lexical shadowing in blocks, closures,
loops, patterns and generic parameters; it does not infer field chains or resolve
compiler identities. Scope traversal and emitted hints are bounded per file.
Unavailable/uncertain annotations retain possible calls and report limitations.
Receiver names and a file merely mentioning an owner do not establish a call.

Context JSON includes `scope`, also rendered in text: admitted indexed files,
live BLAKE3 validation at read time and relationships that are not type-resolved.
Import aliases, trait dispatch, cfg selection and inferred receiver types remain
partial. Caller precision must be reported alongside site recall and confident
function/site coverage; abstaining more is not by itself a complete quality win.

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

Superseded on 2026-10-12: ranking is now the two-stage definition-first plus
coverage/BM25 scheme described under *Implemented paths*; an oversized frame
is answered with `-32600` and the stream continues. Measured effects are in
`validation/FINAL_VALIDATION_REPORT.md`.
