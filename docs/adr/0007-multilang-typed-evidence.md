# ADR-0007: Versioned multilingual typed semantic evidence (Section 4)

**Status:** Experimental until independent indexer-backed accuracy and Windows CI
acceptance. This extends ADR-0006 without changing its existing file-impact
semantics or the official Claude Code launcher.

## Evidence contract

Findex accepts explicit SCIP protobuf indexes from *any language indexer*
and normalized captured-LSP reference snapshots. It does **not** invoke,
install, or silently trust remote indexer processes.

SCIP provides canonical opaque symbols across languages, source occurrence
role bits (definition, import, ordinary reference) and explicit producer
relationships (`is_implementation`, `is_reference`,
`is_type_definition`, `is_definition`). These are stored as distinct,
source-backed observations. LSP captured references are exposed as references
to an exact normalized definition anchor; those are **not** calls.

Supported imported document labels include Rust, C++, Python and TypeScript
(and any bounded SCIP language ID). Labels describe the source language;
they do not imply that Findex has typechecked a project.

No occurrence is promoted to `CALLS_RESOLVED` on the basis of spelling,
syntax shape, file imports or ordinary references. Missing semantic evidence
remains missing.

## Schema and migrations

`relations.sqlite3` now uses its *own* `PRAGMA user_version=2` (independent
of the lexical `index.sqlite3`). Schema setup/migration is transactional;
newer versions are refused **before** any table modifications. Legacy
`occurrences` and file-impact evidence survive migration. Old snapshots
do not magically gain typed roles; the user must explicitly reimport.

New tables:
- `semantic_sites`: (opaque symbol, relative path, 1-based line,
  0-based offset, source language, definition/import/reference kind)
- `semantic_links`: (owner ID, target ID, producer-declared relationship,
  source file and source language)

Every result rechecks admitted source hashes against the current scanner and
live bytes. Complete SCIP imports replace the graph transactionally; captured
LSP batches merge by per-definition identity. Updated files invalidate their
previous typed evidence. Failure to import cannot publish partial evidence.

## API

`findex graph-import-scip index.scip` or
`findex graph-import-lsp captured.json`

`findex graph-symbol "<exact opaque symbol ID>"`

MCP `findex_graph_symbol(symbol, limit)` returns the typed evidence as JSON.
`findex_impact` keeps its original file-level reference semantics.

## Boundaries and pending quality evidence

- SCIP import is capped at 16 MiB, 1,000 documents and 100,000 occurrences;
  relationships are separately capped at 100,000.
- The importer currently accepts compact SCIP ranges and does not claim
  full support for typed_range / all column-offset encodings.
- No native live LSP orchestration, call hierarchy, global inter-indexer
  symbol federation, dynamic dispatch resolution, or verified C++ compilation
  database orchestration is implemented.
- Unit fixtures covering Rust, C++, TypeScript and Python validate
  **ingestion/identity isolation**, not language indexer precision.
- Compiler-backed cross-language precision/recall, source-offset and large
  SCIP streaming benchmarks are required before enterprise release.

References:
- https://github.com/scip-code/scip/blob/main/scip.proto
- https://github.com/scip-code/scip/blob/main/docs/scip.md
- https://microsoft.github.io/language-server-protocol/
- https://www.sqlite.org/pragma.html

No live model sessions or paid APIs are needed for CI.
