---
name: map-project
description: Explore a new or unfamiliar codebase from scratch using Findex and verified source files. Use when asked to understand project architecture, entry points, modules, responsibilities, call relationships, tests, or where to start implementing a feature.
---

# Findex: project discovery

Goal: Build a **bounded, source-backed map for the current task**, not a fictional complete understanding of the repository.

1. Identify stack and workspace boundaries from real manifests (Cargo.toml, package.json, CMakeLists.txt, pyproject.toml, go.mod, etc.), README and tree layout. Use built-in Glob/Read for manifest discovery; Findex cannot guarantee paths in ignored/hidden directories.
2. Ask `findex_context` targeted questions about entry points, startup, configuration, routing, storage, and tests. Prefer several small focused questions over a single massive context dump; start with a 1200–2000 token budget.
3. Call `findex_find_symbol` for exact discovered definitions, `findex_symbols` for selected files, and `findex_search` for implementation concepts. For independent subsystems, gather the relevant evidence separately.
4. Follow verified direct references only. If SCIP/LSP was explicitly imported, `findex_graph_symbol` can show source-backed typed *observations* for an exact opaque ID. Ordinary references are **not** proven calls. For runtime control/data flow, inspect actual implementations and tests.
5. Summarize per subsystem: entry point, responsibilities, dependencies observed, critical data flow (if proven), tests, known gaps, and file/line evidence. Mark uninspected, ignored, too-large and unavailable areas **not assessed**. Never claim 100% mastery.
6. For huge monorepos, scope by package/module and repeat this process locally. If index safety limits are reached or a scan fails, use Claude's built-in exploration and report the limitation. Do not disable index limits or spawn repeated indexing jobs.

Do not run builds, import third-party semantic indexes, modify the repository or launch external model calls as part of discovery unless separately required and authorized.
