# ADR-0006: Evidence-only SCIP/LSP reference graph (first two linked milestones)

**Status:** experimental, importer + reverse file-impact graph. Windows CI is the
acceptance gate. This is not a live LSP client or a compiler-verified call graph.

## Upstream technical contracts

- [SCIP Protobuf specification](https://github.com/scip-code/scip/blob/main/scip.proto)
  and the published `scip` Rust bindings for real `.scip` files: stable opaque
  symbol IDs, `Definition` bit in occurrence roles, 0-based compact source ranges,
  and document-local IDs that **must be scoped by the document**.
- [LSP 3.17 references](https://github.com/microsoft/language-server-protocol/blob/gh-pages/_specifications/lsp/3.17/language/references.md):
  clients query the server for positions. LSP reference responses alone do not
  carry globally unique symbol IDs; a normalized local capture must explicitly
  supply the definition anchor and its reference locations.
- LSP's `textDocument/references` result is not inherently a `calls` edge.
  Findex must not invent one.

## Design

`findex-relations` is independently testable. It accepts a bounded (16 MiB)
user-supplied SCIP protobuf index or a normalized captured-LSP JSON snapshot,
validates every source against the local repository's existing scanner and its
BLAKE3 digest, and updates a dedicated WAL-backed `relations.sqlite3` graph transactionally.
Full SCIP imports replace the graph; individual captured-LSP symbol references
merge by definition-anchor identity, replacing only that symbol. Changed
imported files invalidate every older relation involving them before the new
symbol is recorded. A failed import leaves the last valid snapshot.

The normalized captured LSP JSON accepts a single capture or an array
of captures in one bounded import, avoiding a full workspace reconciliation
for every individual symbol. The single-capture schema is:

```json
{"definition":{"path":"src/a.rs","line":0,"character":3},
 "references":[{"path":"src/b.rs","line":10,"character":5}]}
```

All paths are project-relative, positions are zero-based. Use a trusted language
server to produce the capture; this feature intentionally does not start,
download or trust an arbitrary LSP server process. Each LSP capture represents **one** symbol's reference response. Repeated
captures for distinct definition anchors accumulate into the same graph, but
there is still no automated language-server orchestration or full-project
snapshot scheduling.

For SCIP, only explicit occurrences are imported. Occurrences with the SCIP
Definition role create definition records; other occurrences are references
(to distinguish write/import roles a richer adapter is planned). SCIP local
symbols are keyed with their containing path. Import fails closed on invalid
paths, malformed ranges, oversized evidence or ignored/secret/unindexed files.
Producer-supplied `Document.text` is never trusted or displayed.

The graph query starts from definitions in a target file and finds files with
explicit occurrences referring to those exact semantic IDs. It returns the
source file, 1-based evidence line, defining file/line and provenance.
A depth-1 record is reference-backed. Depth-2/3 traversal is only **potential
transitive file impact** because a referencing file's unrelated definitions
could also have references. Never present this as exact caller chains.

## Performance and safety budgets

- 16 MiB serialized input, 1,000 documents, 100,000 occurrences, 2,048-byte
  symbol IDs, 512 KiB eligible source files.
- Maximum graph depth 3, at most 64 returned files, 4,096 candidate rows per
  expansion, deterministic sort, duplicate/cycle protection.
- Local SQLite transaction; no network calls or dynamic execution.
- Every graph result checks the live BLAKE3 digest of its source and target.
  Modified source evidence is suppressed; modified target causes an error.
- The index importer reconciles the scanner before validating each file;
  repositories and ignored files have the same admission policy.
- LSP captured line and UTF-16 character offsets are bounded against the
  hash-validated actual source; cross-file references retain per-symbol provenance.
- Graph traversal uses a bounded candidate window, and returns `truncated=true`
  when the expansion budget may omit lower-ranked witnesses.
- Source and reference assertions are tested using real serialized SCIP
  Protobuf fixtures and normalized captured LSP snapshots. Untrusted imports
  never overwrite the previous valid snapshot on rejection.

## Not implemented yet

Live language-server lifecycle, versioned documents and incremental LSP updates,
streaming SCIP document decoding for very large projects, importing many LSP
symbols into one snapshot, semantic type/call hierarchy, cross-indexer federation,
precise span/column validation in all encodings, authoritative compiler dependency
edges, A/B retrieval and task-token benchmarks. This is intentionally a
**verified semantic reference and file-impact slice** of those two milestones.
