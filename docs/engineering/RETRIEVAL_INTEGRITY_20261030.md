# Retrieval integrity checkpoint — 2026-10-30

## Starting point and purpose

The checkout started at `28842c5` on `feat/symbol-field-live-lane`, after the R12 Codex navigation
study. The old HANDOFF named `351a3da`, so it was not an accurate description
of the code being continued. Existing untracked October 29 runs were preserved.

Findex's goal remains accurate source localization under a small context
budget, trustworthy compiler/LSP evidence, and bounded local resource use.
Navigation pilots do not establish bug-fixing success, semantic completeness or
arbitrary monorepo scalability. The historical 65-issue localization targets
remain 60% hit@1 and 90% gold-file recall@10; neither is met.

The investigation used Graphify's scoped context query, Findex's connected
context/search/symbol tools, and exact source/caller reads. A broad Findex
query returned tool instructions rather than the implementation; narrowing to
an exact definition located `search_ranked_in_db`. This is a real usability
observation, not a new retrieval-quality benchmark. The candidate was also
queried over its own working source through a fresh native MCP process.
The measurements below needed no new live benchmark session, paid API or credential
access; the later Codex plugin install and four-start paired bugfix pilot are recorded in
`validation/results/20261030-retrieval-delivery/NOTE.md`. The named `codex-max` skill is unavailable at the installed skill
locations checked; local source instructions and bounded workflows were used.

## Changes and engineering choices

| Boundary | Reproduced defect | Implemented repair | Proof |
|---|---|---|---|
| Task localization | `ContextReport.ranked` was rendered in text but discarded by JSON serialization; the offline evaluator reconstructed order from budget-packed snippets | Export the existing `ranked` list. The evaluator consumes that prefix, then distinct evidence paths; old binaries without the field still work. Reject malformed ranked lists | Rust serialization regression; Python ordering/error regressions; paired 65-task run |
| Semantic identity lookup | `load_symbol_sites` silently discarded stale sites and extra results. A small result limit also hid valid sites behind stale rows | Propagate stale counts and truncation into identity and position reports. Examine at most 128 sites plus one overflow sentinel, refill from verified live sites, and keep caps explicit | Two red/green Rust regressions; paired native MCP stale-prefix oracle; six real rust-analyzer SCIP cases |
| Persistent indexing | Excluded build events were filtered only after entering the 2,048-event queue; a build storm set overflow and forced a whole-project reconciliation | Apply the shared exclusion predicate before enqueueing. Errors, rescan flags, Git changes, mixed paths, outside paths and uncertain paths still reach conservative recovery | 16,384 excluded events through the production enqueue path leave room for a source edit; real overflow still signals repair |
| Validation isolation | The hardening fixture inherited the parent Git root when TEMP was inside this checkout, accidentally checking the repository index | Give both authored fixture projects explicit `.git` roots | Original run failed with 839 indexed files instead of one; repaired native hardening passes all eight checks |

The lexical ranker, its weights, exact search/symbol order, dependency set and
MCP protocol remain unchanged. Returning a source-backed reference still does
not prove a call or runtime dispatch. A resolved position may have a truncated
evidence list; the outer and nested reports now both disclose that fact.

## Evidence and interpretation

All evidence for this checkpoint is under
[`validation/results/20261030-retrieval-integrity`](../../validation/results/20261030-retrieval-integrity).
The baseline is the release executable present at session start, frozen before
building. Its SHA-256 is
`6caac63726ca6e0f0ce3906ad91a9fc3d2e5a2f80056aa3a12a199eaeee59f47`;
it was not independently rebuilt from a source commit in this run. Candidate:
`6528a1423f14231ea6a340f97cbd0d53f1378e12edc6e1c8cc0a7c5c35271b68`.
These identify the actual tested executables, not a future release package.

- Native Windows fmt, Clippy with warnings denied, all-target Rust tests
  (**249 passed**) and release build pass. Offline Python tests: **63 passed**.
- **115** frozen navigation controls have identical baseline/candidate native
  MCP hit arrays across requests, zod, fmt and ripgrep. Corpus commits are
  checked against the golden data before comparison.
- **12** self-hosted definition/budget checks pass at 200/500/1,200/2,000 tokens.
  At very small budgets a whole definition can be omitted with explicit
  truncation; these checks do not promise evidence at every budget.
- Six authored Rust scenarios use real rust-analyzer-produced SCIP, then CLI
  and MCP lookup: import aliases, shadowing, outer bindings, generic trait
  declarations, function aliases and local closures all pass. This is not
  general semantic precision/recall evidence.
- The paired native MCP oracle puts two stale references before a live
  definition at result limit one. Baseline returns no sites, stale count zero
  and no truncation. Candidate returns the correct definition and stale count
  two. No lexical fallback or inferred identity is involved.
- Eight native corruption, fallback, capability and project-scoped forget
  checks pass after fixing the fixture Git roots.

### Localization: 65 exposed historical tasks, 1,200 tokens

| Metric | Frozen binary | Candidate |
|---|---:|---:|
| Hit@1 | 58.46% | 58.46% |
| Hit@3 | 78.46% | 76.92% |
| Hit@10 | 89.23% | 89.23% |
| Gold recall@10 | 86.92% | 86.92% |
| MRR | 0.696150 | 0.694868 |

The only first-gold-rank change is `fmt-2695`, 3 to 4. Explicit ranking includes
a file whose snippet did not fit the budget; the old JSON evaluator omitted
that file. The candidate now reflects the ranking already exposed in text.
This interface/evaluation correction is not an accuracy improvement: hit@3
and MRR decrease slightly. No labels, split, weights or budgets were tuned.
The set has been exposed in earlier rounds and is not a fresh blind holdout.
Concurrent validation and ordinary filesystem caches make these timings
unsuitable for a speedup claim.

### Native synthetic scaling

The existing scale harness completed 200 / 2,000 / 10,000 / 100,000-file cases.
The smaller sizes use three paired fresh-index repetitions and equal native
MCP hit lists; 100k is candidate-only, one repetition.

| 100k-file observation | Measured result |
|---|---:|
| Fresh-index wall time (not a flushed OS cache) | 81.063 s |
| Cold-index peak working set | 23.78 MiB |
| MCP process peak working set after warm queries | 39.68 MiB |
| Warm exact-symbol p95 (15 samples) | 12.413 ms |
| Warm lexical p95 (15 samples) | 3.188 ms |
| Common-term p95 (5 samples) | 61.193 ms |
| Idle CPU over five seconds | 0.0 s |

Warm controls require `sync.examined == 0`, correct file/line results and
explicit common-term truncation. Adding file 100,001 is rejected; all nine
selected old digests remain unchanged, the index integrity check passes, and
removing the excess file admits the nine pending edits. The completion marker
records PASS. Concurrent validation and a synthetic one-function-per-file
corpus preclude general latency, speedup, language or production-soak claims.

### Environment and limits

Initial default TEMP/LOCALAPPDATA runs failed with access denied. Relocating
them under this checkout exposed the inherited broad-read ACL caught by the
existing cache-security test. A new dedicated test-state directory received
private ACLs; the security test itself was not weakened. Both initial failures
and the successful controlled run are retained. User cache ACLs were unchanged.

Graphify was refreshed with `graphify update .` using AST extraction only.
The final refresh reported 10,448 nodes / 12,401 edges and an unrepresented Cargo.toml; the
manifest was read directly. Community labels were not regenerated with a model.

## Next engineering gates

1. **Defect localization:** measure candidate recall before reranking on a new,
   frozen independent issue set. Separate failures caused by missing candidates,
   wrong ordering and snippet packing. Gate every new feature on recall and
   false negatives, not improvements on this exposed 65-task set alone.
2. **Deep semantics:** add a compiler-backed context adapter above the index and
   relations crates, preserving their dependency boundary. Use explicit symbol
   identities, source digests, ambiguity and provenance. Evaluate precision,
   recall and confident coverage together, including aliases and dispatch;
   syntax/name matches must remain visibly separate. Review alias traversal
   bounds/provenance before relying on arbitrary imported alias graphs.
3. **Large workspaces:** existing new-file/rename/ignore/branch changes still
   require full reconciliation, and standalone semantic lookups reconcile the
   project. Next is measured bounded dirty-path scheduling and snapshot
   generations, then partitioning if profiles justify it. Preserve ignore-rule
   correctness, overflow recovery and transactional cap rollback. Synthetic
   100k-file evidence cannot establish arbitrary-size, SMB or OneDrive safety.

No milestone, signed release, broad competitor win or fresh end-to-end model
bug-fixing improvement is claimed by this checkpoint.

An independent final reviewer examined the changed code and immediate callers
and reported no actionable introduced defect. This bounded review is separate
from the measured gates and is not a whole-repository audit.
