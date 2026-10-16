---
name: maintenance
description: Diagnose or explicitly maintain the local Findex index and SCIP/LSP imports. Use only when the user explicitly asks for Findex setup, index health, rebuilding, or semantic evidence import.
disable-model-invocation: true
---

# Findex: operator-controlled maintenance

This is an **explicit-user-only** skill. Findex MCP search tools are read-oriented, but indexing and importing modify the local derived cache.

- Check the native runtime: `findex doctor`, `findex --version`. If installed as a self-contained plugin, use the bundled `bin/findex.exe` in the plugin's actual directory instead of guessing PATH.
- Before assuming an index is healthy, use `findex index-check` on the current project. This verifies structural consistency; it does not prove every filesystem event was observed.
- `findex index` reconciles the repository and may take time. `findex refresh <known path>` is for already indexed files and does not admit new ones.
- `findex graph-import-scip <index.scip>` and `findex graph-import-lsp <capture.json>` import **explicit, trusted local files**. Do not fetch or generate these silently; confirm the intended file and user's request first.
- Never weaken ignore rules, enlarge limits blindly, delete user data, install binaries, switch authentication providers, or make network calls to solve a missing runtime.
- On failure, explain the problem and continue with Claude's built-in tools where possible.
