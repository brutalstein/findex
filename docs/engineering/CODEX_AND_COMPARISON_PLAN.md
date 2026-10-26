# Codex integration and three-tool comparison

## Design

Use the existing Windows `findex.exe mcp` engine in both clients. Add Codex
plugin metadata/MCP configuration to the existing plugin package and make its
four skills client-neutral. Keep Claude's manifest/configuration compatible.
Install a self-contained local Codex plugin with the verified release binary.
No new network service, model provider, credential access or retrieval change.

Alternatives: a standalone `codex mcp add` registration is simpler but does not
distribute the requested plugin skills; a separate Codex engine duplicates
working protocol code. The shared plugin/engine is the proposed approach.

## Measurement contract (freeze before running)

- Compare Findex, Graphify code-only and codebase-memory-mcp as installed,
  with recorded versions and native Windows commands. Report Graphify's local
  AST mode explicitly; do not call it equivalent to semantic LLM extraction.
- Reuse pinned public corpora and frozen golden answers. Select at least 60
  held-out queries by a deterministic, balanced rule before querying tools.
  Record the selected IDs and a dataset SHA-256. Do not tune on those answers.
- Use supported native search operations, identical tasks, candidate limits
  and a common output budget. Include errors/misses in denominators. Record
  raw outputs and commands; no manual result selection or oracle reranking.
- Report definition/file/line retrieval accuracy, false positives, latency,
  returned context tokens using a named local tokenizer, index time and disk
  footprint separately. Output tokens are a context-cost proxy, not a bill.
- A separate live pilot uses the same two frozen navigation tasks per client
  across three tools: at most 12 sessions total, sequential, subscription only.
  Reuse Claude's billing gate; require Codex ChatGPT login and remove API-key
  routes from test environments. No credential files are read. Stop on usage
  limits, authentication failures or ambiguous billing. Count actual usage,
  tool use, grounded correctness and client/version; no cross-model averaging.
- Findex passes the requested superiority gate only if it uses fewer measured
  tokens and achieves higher accuracy/task success than **each** competitor.
  Ties, missing measurements and inconclusive small samples do not pass.
  Publish losses and limitations. A two-task pilot cannot establish a general
  performance claim or statistical significance.
- If improvements are justified by dev failures, retain existing ranking
  reference tests and measurements and use fresh held-out data to validate.

## Implementation and verification

1. Extend `validation/harness/plugin_check.py` to validate both manifests,
   MCP launch configuration and shared skills; observe a failing check first.
2. Add Codex metadata/config and client-neutral skill wording. Test native
   plugin installation in an isolated Codex home, then install the local
   self-contained package for this user. Verify Claude checks still pass.
3. Add one bounded comparison harness reusing existing scoring, MCP and
   result-sanitization helpers, plus focused scoring/adapter self-checks.
4. Run the frozen local comparison, then the subscription pilot within the
   authorized cap. Record any blocked component rather than bypassing a gate.
5. Run Windows fmt, Clippy, tests, release build and plugin checks; update
   Graphify and documentation. Report the measured gate outcome.

Preserve all existing uncommitted release-preparation changes. No publication,
Git push, provider changes, or changes to unrelated tool registrations.
