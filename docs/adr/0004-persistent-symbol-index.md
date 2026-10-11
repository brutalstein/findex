# ADR-0004: Persistent content-addressed definition index

**Status:** first experimental slice, Windows CI validation required.

The `findex-index` module owns two additive SQLite tables:

- `symbol_state(path, hash, extractor_revision, language, has_syntax_errors, truncated)`
- `symbols(path, ordinal, name, kind, start_line, end_line)`

`symbols_by_name` indexes exact names with SQLite's ASCII `NOCASE` rules.
The full scanner and targeted known-path refresher share the exact same
`symbols::upsert` code path. A BLAKE3 digest **and** extractor revision must
match to reuse a parsed source; otherwise existing records are cleared and
repopulated. Unsupported file extensions have no symbol entries.
Deletion and binary/unreadable transition clear the records atomically alongside
the lexical documents. A migration from an older lexical-only database is
implemented by parsing eligible existing files when `symbol_state` is absent.

Readers join symbol rows to the currently stored file digest, and verify the
actual on-disk file hash before returning a source span. Stale matches are
never returned. A line span is syntactic evidence, not confirmed caller,
implementation or type resolution. Parser syntax errors/truncation are surfaced.

This slice does not persist complete ASTs, does not implement incremental
Tree-sitter edit/old-tree reuse, and does not provide LSP/SCIP references.
Re-parsing entire *changed files* is deliberately chosen for simple and
verifiable correctness at the existing <=512 KiB/file boundary. Tree-sitter's
optional previous-tree reuse can be introduced only with exact edit ranges and
a benchmark showing net benefit.

A warmed symbol-index query still reconciles the workspace before CLI/MCP
search; avoiding that walk is the next incremental watcher integration step.
No search quality / latency / token savings are claimed without measurement.

References:
- https://tree-sitter.github.io/tree-sitter/using-parsers/3-advanced-parsing.html
- https://www.sqlite.org/fts5.html

## Watcher-backed follow-up

In the MCP process, lexical, exact-symbol and single-file AST tools share
the same lazily initialized watcher session. `findex_find_symbol` calls the
incremental event-reconciliation path, then queries the persisted symbol
tables; warm queries therefore skip a complete workspace traversal.
`findex_symbols` checks its path against the **indexed admission set** and
current hash, refusing ignored, secret-named or stale sources before parsing.
The standalone CLI `findex symbols` reconciles fully for a reliable policy
snapshot. All paths keep bounded file reads.

The watcher is eventual-consistency, not a proof of instantaneous event
delivery on network/OneDrive volumes. Unknown paths and event loss trigger
full reconciliation, and all output still verifies source digests.
