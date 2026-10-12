# Research notes

Primary sources only; each finding was checked against a local probe.

| Question | Finding | Source / probe |
|---|---|---|
| Why does using Findex cost an extra turn? | Claude Code defers MCP tools behind `ToolSearch` when tool search is active; the model must search before the first call. | code.claude.com/docs/en/mcp; `toolsearch_calls` in every B/C session that used Findex |
| Can a server make one tool load upfront? | Yes: per-tool `_meta: {"anthropic/alwaysLoad": true}`. | same docs; probe: `findex_context` present in `init` tools without ToolSearch |
| Description length | Tool descriptions are truncated at 2048 characters. | same docs |
| Does a skill avoid the turn? | No: invoking a skill is itself a tool call/turn. | Claude Code docs (skills); harness counts |
| Isolating study sessions from the operator's instructions | `CLAUDE_CODE_DISABLE_CLAUDE_MDS=1` blocks all CLAUDE.md loading. `--setting-sources project,local` does **not** exclude `~/.claude/CLAUDE.md`. `--safe-mode` also disables MCP. `--bare` requires an API key (not allowed). | probes recorded in the campaign log; CLAUDE_AB_RESULTS |
| Where are tokens spent? | ≈26.5k cache-read tokens per turn (system prompt, schemas, history); tool payloads are a small share. | `results/20261012T001625Z-bf4967d90524` |
| Independent localization labels | Issue title + files changed by the closing commit, evaluated at the parent: a standard construction for file-level bug localization benchmarks (cf. SWE-bench-style task mining). | `harness/issues.py` |
| Compiler-grade reference oracle for Rust | rust-analyzer `scip` emits occurrences with `SymbolRole.Definition` (bit 1); non-definition occurrences are references. | SCIP protobuf schema (sourcegraph/scip `scip.proto`) |

## Ideas evaluated and not adopted

- Embeddings / semantic search: no measured failure required them (D3).
- Heuristic call graph from Tree-sitter: would carry the same name ambiguity
  measured in GRAPH_RESULTS under a more confident label.
- Making all tools `alwaysLoad`: adds schema tokens to every turn of sessions
  that never need Findex.
