---
name: trace-impact
description: Trace likely code dependencies, usages, tests and possible change impact with Findex. Use when asked what calls a function, which modules depend on a file, what a refactor could break, or how changes propagate across modules.
---

# Findex: conservative relationship and impact tracing

1. Locate the **actual declaration** and enclosing implementation with `findex_context` or `findex_find_symbol`. Inspect the source using Read before claiming semantic relationships.
2. For lexical uses, call `findex_context` using an exact identifier; respect candidate-window/truncation warnings. Lexical sites are not resolved calls.
3. If a verified SCIP/LSP snapshot was already imported, use `findex_impact` for affected files or `findex_graph_symbol` with an exact opaque symbol ID. Report producer, path and hash-backed evidence where available; classify multi-hop dependencies as **possible** impacts.
4. Check dynamic dispatch, overloaded names, generic implementations, macros, generated code and conditional compilation manually with the compiler or tests where appropriate. Use Grep/Read when Findex cannot prove a relationship.
5. Report: confirmed source facts, likely impacts, ambiguous or unknown edges, recommended tests, and index freshness/coverage limitations. Do not state that a source-level reference proves execution.
6. Never automatically run `graph-import-scip`, `graph-import-lsp`, rebuild the index, or approve edits solely to increase graph coverage. Those are explicit operator actions.

When Findex is unavailable, continue with built-in Claude Code navigation and report that relationship confidence is limited.
