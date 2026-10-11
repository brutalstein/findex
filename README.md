# Findex

**Native Windows code discovery for official Claude Code.**  
**Status: experimental v0.1 foundation; not production-ready.**

Findex is designed as a local-first, modular code intelligence engine that helps the official Claude Code client retrieve small, verifiable code contexts instead of blindly reading whole projects. It does **not** emulate Claude, proxy requests, collect Claude credentials, or claim 100% understanding/token savings.

## First working slice

- Native Rust CLI for **Windows 10/11 x64** (`findex.exe`).
- `findex claude [flags...]` launches the official `claude.exe` in the existing terminal and working directory, forwarding arguments.
- Optional adjacent Claude plugin contributes a `findex_search` MCP tool using the normal Claude Code plugin loader.
- Lexical search: SQLite FTS5/BM25, ignored directories, bounded per-file scan, stable local index path, atomic file updates and incremental content hashes.
- Every retrieved hit checks current file content against its indexed hash, avoiding publication of stale snippets.
- Indexing does not upload source; excludes common secret filenames but is **not a secret scanner**.
- Native Windows CI: format, clippy, tests, optimized build and artifact packaging (when pushed to GitHub).

### What is **not** implemented

Tree-sitter AST, LSP/SCIP navigation, dependency and cross-language graphs, active Windows directory watcher, hybrid embeddings, adaptive search planner, verified project memory, source-code secret scanner, token telemetry and A/B benchmarks. Retrieval is currently lexical only. On huge monorepos searches can be slow because each query reconciles the workspace; this is intentional correctness-first bootstrap behavior, not final architecture.

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

- In v0.1, the launcher specifically invokes official **native** `claude.exe`; legacy npm-based `.cmd` entrypoints are not supported. It never silently impersonates or overwrites a Claude executable.
- Findex does not change the user's Claude Code settings or take over authentication.
- Local indexing has a 100,000-file reconciliation safety limit and scans UTF-8 files at most 512 KiB each. These are provisional bootstrap defaults.
- Git-ignore rules, hidden directories and common vendor/build directory filters apply. Ignored files will not be retrieved.
- Findex's MCP adapter implements the established 2024/2025 JSON-RPC stdio handshake; the newest 2026 protocol and official Claude plugin compatibility require actual integration tests.
- `ANTHROPIC_API_KEY` may affect official Claude Code authentication; `findex doctor` warns without logging its value.

## Modules

| Crate | Responsibility |
| --- | --- |
| `findex-core` | Canonical project root, local storage identity, scan policies |
| `findex-index` | Atomic SQLite/FTS5 lexical index and evidence-backed file/line retrieval |
| `findex-mcp` | JSON-RPC over stdio exposing a bounded project search tool |
| `findex-cli` | Windows executable, CLI commands, official Claude process launching |
| `plugin/` | Official Claude Code plugin manifest and MCP registration |

## Why measurements matter

A smaller snippet is **not** verified token savings. Findex will eventually measure total model input/output/cache tokens and task success in matched runs with and without Findex, and compare against established code navigation tools. Until then the CLI deliberately shows token savings as unavailable.

See [Architecture](docs/architecture.md), [Roadmap](docs/roadmap.md), [Security](SECURITY.md), [Contributing](CONTRIBUTING.md), and [ADR-0001](docs/adr/0001-native-cli-and-local-index.md).

## License

MIT. Findex is an independent project and is not affiliated with Anthropic or the Claude brand.
