# Findex hardening: fail-closed failures and compatibility evidence

Scope: source-backed error handling, index corruption safety, operator health
checks, and an optional static MCP capability contract. This is a security
hardening stage **not** a claim of universal zero-risk operation.

## Changes and guarantees within test scope

1. MCP tool execution failures use stable bounded messages. Absolute path,
   username, raw SQLite errors and credentials from lower-level errors are
   not interpolated into content returned to Claude. The tool returns
   `isError=true` and instructs Claude to use built-in navigation.
2. Corrupt `index.sqlite3` is not silently deleted. Findex fails closed,
   leaving source files and the corrupt cache untouched for explicit operator
   investigation. This intentionally changes earlier auto-repair behavior.
3. Operator `findex index-check` now uses full SQLite `integrity_check`
   instead of `quick_check`, followed by FTS5 consistency and source-to-FTS
   rowid agreement. It is intentionally not part of normal queries.
4. Optional `findex_capabilities` MCP tool exposes supported language
   syntax, project scan limits and absence of a complete resolved call graph.
   The contract is static and requires no scan, network or model call.
   Retrieval/indexing run locally, but selected excerpts returned to Claude
   may reach the model provider; this is not local-only inference.
5. CI runs an isolated real binary test for corruption preservation,
   internal schema checks, privacy-safe MCP error, capability discovery and
   source retention. Existing 1,200- and 10,000-file Windows stress suites,
   Rust 1.85 tests, all Rust targets, and plugin six/seven-tool smoke continue.

## Deliberately not promised

- Encryption at rest, perfect secret detection, audited Windows DACL isolation.
- Automatic safe recovery or rotating snapshots while another Findex instance
  has a SQLite WAL open. Recovery is an explicit operator task after backup.
- Perfect graph type resolution in Rust, C++, Python, TS; every possible
  filesystem / OneDrive / SMB condition; fixed memory limits at 100k-file scale.
- Fully automatic system-wide optimization or measured universal token savings.

Always classify these as **OPEN** until reproducible Windows fixtures and
independent adversarial benchmarks pass. No paid model tests are performed.

Reference: https://www.sqlite.org/pragma.html for the semantic difference
between quick_check and integrity_check.
