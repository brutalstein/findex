# Architecture decisions

Full rationale and evidence: `validation/research/ENGINEERING_DECISIONS.md`.

## Module placement

| Component | Crate / file | Boundary kept |
|---|---|---|
| Context engine (`context`, `context_in_db`, `ContextReport`, `Evidence`) | `findex-index/src/context.rs` | Reads only the project's own index and files; every cited file is re-hashed |
| Enclosing-symbol lookup (`containing`), `DefinitionRow.end_line` | `findex-index/src/symbols.rs` | Filtered by extractor revision and file hash |
| `findex_context` MCP tool | `findex-mcp/src/lib.rs` | Appended last in `tools/list`; stdout stays JSON-RPC only |
| Watcher-backed context | `findex-watch/src/lib.rs` (`SearchBackend::context`, `WatchedIndex::context`) | Falls back to a full scan if the watcher fails |
| CLI | `findex context [--budget N] [--json] <task>` | — |
| Relations DB open retry | `findex-relations/src/lib.rs` | Same retry rule as the lexical index |

## Summary of decisions

1. One task-level tool, loaded upfront; existing four tools unchanged and
   deferred (backward compatible).
2. Deterministic routing and retrieval; no model, no embeddings, no network.
3. Evidence is labelled by how it was found (`definition` from Tree-sitter;
   `call`/`reference`/`import` are name matches); ambiguity and truncation are
   reported as `missing:` lines, never hidden.
4. Budgeted packing: priority definition > (tests when asked) > call/match >
   reference > import > prose; concept tasks rank test code after source.
5. Untrusted repository text is indented and labelled so it cannot pose as
   report structure.
6. No knowledge/memory layer was added. Persisting "project facts" would
   create answers that can go stale without a hash check; nothing measured
   required it.
