# Source-verified project overview — experimental, on-demand

`findex overview` and optional MCP `findex_overview` build a bounded
file inventory for a new or unfamiliar project. They perform a full safe
reconciliation *only when invoked*, then inspect the current lexical index.

The JSON report includes:
- Indexed/admitted file count, probable test-file count.
- Language distribution inferred from **extensions only**.
- Top 16 directory/module clusters by indexed file count.
- Up to 20 manifest and up to 20 *candidate* entrypoint paths, only when
  the live file bytes still match their indexed BLAKE3 hashes.
- Truncation and stale-landmark indicators, scan statistics and evidence scope.

This is NOT proof of control flow, architecture, interface contracts or all
source files in a monorepo. Ignore rules, 100k-files and 512KiB-per-file
bounds apply, and a race can change sources after inventory. Claude should
read chosen manifests and source before using them to make conclusions.

### Why it matters

A Claude coding agent can begin with a small deterministic module map
instead of recursively reading every file. It can then query `findex_context`
for targeted semantic evidence, only explicitly import SCIP/LSP when asked,
and fall back to built-in Read/Grep/Glob for gaps. This is an
incremental improvement to first-contact discovery, not verified persistent
project memory.

### CI evidence

`validation/harness/project_overview_smoke.py` tests a local mixed Rust /
TypeScript / Python workspace with secret-file exclusion, source-hash-backed
manifest and entrypoint hints, on-demand real MCP access and deletion/refresh
correctness. Zero model sessions, no paid API calls.

Unmet guarantees: per-module data flow, automatically recovered project
knowledge, verified semantic call graph, 100k-file production resource limits,
SMB/OneDrive/WSL compatibility, universal token savings.
