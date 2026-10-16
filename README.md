# Findex

**Native Windows code discovery for official Claude Code.**  
**Status: experimental v0.1 foundation; not production-ready.**

Findex is designed as a local-first, modular code intelligence engine that helps the official Claude Code client retrieve small, verifiable code contexts instead of blindly reading whole projects. It does **not** emulate Claude, proxy requests, collect Claude credentials, or claim 100% understanding/token savings.

## First working slice

- Native Rust CLI for **Windows 10/11 x64** (`findex.exe`).
- `findex claude [flags...]` launches the official `claude.exe` in the existing terminal and working directory, forwarding arguments.
- Claude Code native plugin works with **official `claude` directly** (no `findex claude` required): on-demand navigation/project-map/impact skills and the MCP tools `findex_context` (discoverable upfront), `findex_search`, `findex_find_symbol`, `findex_symbols`, `findex_impact`, `findex_graph_symbol`. Self-contained signed-checksum ZIP artifacts and a source marketplace are available; the native binary is unsigned.
- Task-oriented context (`findex context <task>`, MCP `findex_context`): for a named identifier, its definition, every call site with the enclosing function, or its tests; for a named file, its definitions; otherwise a text search over the task words. Intent words are recognised in English and Turkish. A report with a coverage line and no truncation lists every code occurrence of the name; this claim is checked against `git grep` by `validation/harness/coverage_check.py`.
- Definition-first retrieval: queries matching a defined identifier (`GitignoreBuilder`, `gitignore builder`, `gitignore_builder`) return the definition first; other queries use SQLite FTS5 with camelCase-aware terms, ranked by query-term coverage, same-line co-occurrence and BM25. Every returned hit is source-hash verified; excerpts mask common credential shapes.
- Persisted Tree-sitter definitions with language-aware extraction for Rust, Python, TypeScript/JavaScript and C++.
- Native Windows file watcher and incremental refresh of indexed files, with conservative full-scan fallbacks.
- Explicit SCIP protobuf imports and mergeable, captured LSP symbol-reference evidence. Neither launches an LSP server.
- Reverse file-impact lookup using source-backed semantic references; indirect impacts are only *potential*.
- Indexing does not upload source; excludes common secret filenames but is **not a secret scanner**. Text of removed or excluded files is zeroed from the index database (SQLite `secure_delete`, FTS5 `secure-delete`).
- Native Windows CI: stable/MSRV Rust, format, lint, tests, build, MCP smoke and unsigned ZIP packaging.

### What is **not** implemented

Live LSP orchestration, complete cross-language/type/call graph, hybrid embeddings, a general adaptive search planner, verified project memory, full source-code secret scanning (only excerpt redaction of common credential shapes exists) and token telemetry inside the CLI. Retrieval, performance, security, real Claude Code integration and a small matched Claude Code A/B study are measured by the [validation suite](validation/README.md); see its report for what was and was not shown. Findex is still experimental. CLI searches reconcile before each query; warm MCP lexical and symbol lookups use a native watcher with safe fallbacks.

## Claude Code native plugin (no `findex claude` required)

Findex can run as a **normal Claude Code plugin**, without opening a Findex launcher. It adds six local MCP tools for context, lexical search, exact symbol lookup, per-file syntax, optional reference impact and typed symbol evidence, plus on-demand skills for code navigation, mapping an unfamiliar project and tracing change impact. Claude's built-in Read/Grep/Glob remain available and should be used whenever Findex evidence is missing, incomplete or not enough to justify editing.

**Recommended evaluation:** obtain the SHA-256-verified `findex-claude-plugin.zip` artifact from a successful Windows CI run, extract its `findex-plugin` directory, then run the official `claude --plugin-dir "C:\\Tools\\findex-plugin"` in any repository. This artifact includes `bin/findex.exe`; it does **not** download or install binaries at runtime.

**Developer/marketplace installation:** `claude plugin marketplace add brutalstein/findex`, then `claude plugin install findex@findex-marketplace`. The source marketplace provides plugin files **only**. Configure an existing verified `findex.exe` on PATH or set `FINDEX_BIN` to its absolute path. The plugin never secretly rebuilds or installs native code.

Try `/findex:navigate`, `/findex:map-project`, `/findex:trace-impact` or ask Claude naturally (English or Turkish). The maintenance skill requires an explicit user invocation. Technical boundaries and verified installation steps: [plugin/README.md](plugin/README.md).

**Trust boundary:** Tools report hash-verified *admitted* source evidence, not an exhaustive model of all code or type-resolved call edges. No plugin can guarantee zero risk or guaranteed token savings on all project sizes; large repositories, ignored files and incomplete indexers need conservative fallback and separate evidence.

## Build

Prerequisites: Windows 10/11 x64, recent stable Rust and Cargo (MSVC target). A native installation of official Claude Code is needed for `findex claude`; it must expose `claude.exe` on `PATH`.

```powershell
cargo test --workspace --all-targets
cargo build --release -p findex
```

For plugin-assisted Claude Code, copy `plugin/` **beside** `findex.exe` and put that executable folder on `PATH` (or invoke with its full path). The plugin MCP config invokes `findex.exe mcp` as a child process. The launcher prepends its executable directory to the child's `PATH` to keep that command resolvable.

```powershell
cd C:\Projects\YourProject
findex doctor
findex index
findex search validate token
findex claude
findex claude --resume
```

If Findex's bundled plugin is not present, the official Claude Code still launches, but will not have a Findex search tool. Do not infer MCP usage from installation alone: inspect Claude's tools and benchmark actual task outcomes.

### Important compatibility scope

- The launcher specifically invokes official **native** `claude.exe`; legacy npm-based `.cmd` entrypoints are not supported. It never silently impersonates or overwrites a Claude executable.
- Findex does not change the user's Claude Code settings or take over authentication.
- Local indexing has a 100,000-file reconciliation safety limit and scans UTF-8 files at most 512 KiB each. These are provisional bootstrap defaults.
- Git-ignore rules, hidden directories and common vendor/build directory filters apply. Ignored files will not be retrieved.
- Findex's MCP adapter serves both the `initialize` handshake (2025-11-25, 2025-06-18, 2025-03-26, 2024-11-05) and the stateless 2026-07-28 protocol (`server/discover`, per-request `_meta` version). Claude Code 2.1.281 was tested for real via `--mcp-config`, `--plugin-dir` and `findex claude`; it negotiates 2025-11-25.
- Build with the rustup MSVC toolchain (`rust-toolchain.toml`). A standalone `gnullvm` Rust earlier on `PATH` produces a `findex.exe` that needs `libunwind.dll` and fails to start as an MCP server without it.
- `ANTHROPIC_API_KEY` may affect official Claude Code authentication; `findex doctor` warns without logging its value.

## Modules

| Crate | Responsibility |
| --- | --- |
| `findex-core` | Canonical project root, local storage identity, scan policies |
| `findex-index` | Atomic SQLite/FTS5 lexical index, persisted definitions, evidence-backed retrieval and the task context engine |
| `findex-syntax` | Bounded Tree-sitter definition extraction |
| `findex-watch` | Native Windows watcher and incremental refresh |
| `findex-relations` | SCIP and captured-LSP reference evidence, file impact |
| `findex-mcp` | JSON-RPC over stdio exposing bounded project tools |
| `findex-cli` | Windows executable, CLI commands, official Claude process launching |
| `plugin/` | Official Claude Code plugin manifest and MCP registration |

## Why measurements matter

A smaller snippet is **not** verified token savings. The validation suite measures total model input/output/cache tokens and task success in matched Claude Code runs with and without Findex ([report](validation/FINAL_VALIDATION_REPORT.md)). The CLI still shows token savings as unavailable: a small study on a handful of tasks does not justify a per-session savings figure. The validation runner never starts a Claude session by default; live model tests need an explicit flag and approval ([validation/README.md](validation/README.md)).

See [Architecture](docs/architecture.md), [Roadmap](docs/roadmap.md), [Security](SECURITY.md), [Contributing](CONTRIBUTING.md), and [ADR-0001](docs/adr/0001-native-cli-and-local-index.md).

## License

MIT. Findex is an independent project and is not affiliated with Anthropic or the Claude brand.

### Incremental refresh (foundation)

After an initial `findex index`, `findex refresh src\\app.rs` updates **already
indexed** relative file paths through one SQLite transaction without walking the
whole project. Deleted or newly binary files lose their indexed entries. Paths
that are not already indexed are reported as **unknown** and require
`findex index` to discover them; this is intentional so arbitrary paths cannot
bypass Git exclusions or secret-file filters. An invalid or escaping path aborts
the entire batch. A known file that cannot be read for a reason other than
deletion (for example locked by an editor mid-save) keeps its entry; hits are
re-verified against the live file hash, so it is never returned unverified.
CLI searches perform a full reconciliation; the MCP server uses the native
watcher described below.

## Native watcher-backed MCP (development milestone)

The long-lived `findex mcp` process now initializes a Windows directory watcher on
its first search. Common content edits update known paths through one transaction,
with duplicate paths collapsed. Creation, deletion, renames, ignore-rule changes,
watcher errors, overflow and a periodic 120-second deadline force a safe full
reconciliation. A bounded channel prevents unbounded event-memory growth.

If the native watcher cannot be started, Findex falls back to per-search full
reconciliation. The CLI `findex search` remains a correctness-first full scan.
All hits still validate current file hashes before returning evidence. Native
filesystem notifications can be missing on remote/sync-backed filesystems; Findex
is *eventually consistent* for newly added/changed matching files between
reconciliations, not a 100% synchronous snapshot guarantee. Avoid relying on
this as the sole correctness check when editing critical code.

Windows CI validates event classification, invalidation, protocol negotiation,
lexical fallback and optimized packaging. Real-world perf and race testing on
large repositories is a separate milestone; no token savings are claimed.


### Syntax-aware definition inspection (experimental)

`findex symbols crates\\findex-watch\\src\\lib.rs` uses a bounded local
Tree-sitter parser to return named function, type, class and method definitions
with source line ranges. The adjacent Claude plugin also exposes
`findex_symbols` (one project-relative path per call). Rust, Python,
TypeScript/TSX/JavaScript-family and C++ are supported initially.

This **does not** infer callers, implementations, types, or semantic
cross-file references. Unsupported languages report an explicit fallback,
and files with syntax errors report that limitation. The analysis is
on-demand; project-wide definitions are persisted by the index described
below. No extra model tokens or remote service are used by the parser itself.

## Persistent AST definition index (experimental)

Findex now stores syntax definitions in `symbol_state` and `symbols` SQLite
tables, using the file's BLAKE3 digest and an explicit extractor revision as
cache keys. After a first `findex index`, unchanged supported files avoid
re-parsing by Tree-sitter. Lexical documents and symbol records are updated
or deleted in **one SQLite transaction**, including watcher targeted refreshes.
Previously created lexical databases are populated on their next reconciliation.

`findex find-symbol WatchedIndex` looks up exact names across files with
1-based source line ranges and verifies the live file digest before publishing
results. The adjacent MCP tool `findex_find_symbol` does the same. Exact names
are compared with SQLite's ASCII `NOCASE`, not language-specific name resolution.
CLI exact-symbol lookups still perform a full reconciliation. MCP symbol
lookups reuse the **same native watcher session** as lexical retrieval: warm
queries without relevant events avoid a whole-repository traversal.
The persistent symbol records avoid unchanged-file AST parsing.
Files with syntax errors are marked as such; no call relationships are inferred.

### Indexed-only AST security policy

CLI `findex symbols <path>` and MCP `findex_symbols` now only inspect
files admitted by the same code scanner used by the lexical index.
Unindexed, ignored, secret-named, traversal and stale paths cannot bypass
the repository's ignore policy through direct syntax inspection. CLI
reconciles before inspection; MCP processes watcher events and reconciles
periodically. Syntax definitions do **not** guarantee semantic references.

## Windows dependency and build verification

The tracked `Cargo.lock` fixes the resolved Rust dependency graph. Windows
CI enforces `--locked` for dependency resolution, Clippy, tests and native
Release builds. A separate runner checks the declared minimum Rust 1.85.0
compatibility. Release CI provides `findex-windows-x64.zip` alongside its
SHA-256 checksum file; the ZIP includes the build compiler versions and lock
file. This is **not** a cryptographically signed or bit-for-bit reproducible
binary guarantee; see `docs/adr/0005-build-provenance.md`.

## Semantic reference imports and impact graph (experimental)

Findex supports explicit **SCIP protobuf** reference imports and a normalized
**captured LSP references** JSON adapter. Neither runs a language server or
executes downloaded indexers; you generate the source evidence separately.

```powershell
findex graph-import-scip .\index.scip
findex impact crates\findex-index\src\lib.rs
findex impact crates\findex-index\src\lib.rs 2
```

For normalized LSP reference captures, pass a JSON file with a single capture or an array of captures. Batched captures index many separate symbols with one file reconciliation. A
`definition` location and `references` locations, each with project-relative
`path` and zero-based `line`, `character` fields. Then run
`findex graph-import-lsp capture.json`. The MCP tool `findex_impact` exposes
previously imported verified evidence to Claude Code. Direct references are
source-backed; indirect file impact is marked **potential**, never a proven
call graph. SCIP imports replace the full semantic snapshot. Captured LSP imports now merge separate symbol identities, replacing only the same definition anchor and invalidating prior edges that reference updated source content. A capture remains a single-symbol reference set; all imported evidence is bounded and hash-verified.
See `docs/adr/0006-scip-lsp-evidence-graph.md` for format, budgets and limits.
