# Evidence status

Living summary of what Findex has demonstrated, as of **2026-10-26**
(branch `quality/review-fixes`, version 0.2.0). Every number points to a committed run under
`validation/results/` or a campaign record; campaign records themselves are
historical and are not rewritten. Machine: native Windows 11, NVMe.

Verdicts:
- **Proven**: measured on held-out data or against an independent oracle, frozen before the run.
- **Supported**: measured, but only on development data, a single model, or small n.
- **Not proven**: measured, and the claim did not hold.
- **Not run**: no measurement.

## Accuracy

| Claim | Number | Baseline | Evidence | Verdict |
|---|---|---|---|---|
| File retrieval (642 golden queries, 7 corpora) | R@1 0.984, MRR 0.991; held-out R@1 0.974 | start build R@1 0.695; `git grep` R@1 0.790 | `validation/FINAL_VALIDATION_REPORT.md` §1 | Proven (golden set written by the engine author) |
| Owner-qualified callers, ripgrep (rust-analyzer SCIP oracle, held-out split) | call precision **0.714**, site recall **0.981** | 0.523 / 0.957 | `validation/results/20261026-callsites-qualified-final/` | Proven |
| Owner-qualified callers, tokio (same) | call precision **0.567**, site recall **0.947** | 0.376 / 0.829 | same | Proven |
| Unqualified callers, tokio (dev split) | call precision 0.418 | 0.391 | same | Supported |
| Coverage line ("every code occurrence is shown") vs `git grep -w` | 279/279 names, 7 corpora | 268/279 at frozen main (A1) | same, `coverage/` | Proven |
| Issue localization (65 real fixed issues, frozen) | `findex_context` hit@1 0.308, MRR 0.428, gold recall@10 0.662 | `git grep` words hit@1 0.062, MRR 0.253 | same, `localization/` | Proven better than `git grep`; hit@1 is still low |
| Definition lookup vs other tools (60 held-out queries, 4 corpora, same budget) | Findex hit@1 0.967, MRR 0.983, 419 tokens; Graphify (code-only) 0.650 / 0.676 / 1,466; codebase-memory-mcp 0.633 / 0.646 / 355 | — | `validation/results/20261026-codex-comparison/` | Supported: more accurate than both; fewer tokens than Graphify but not codebase-memory-mcp, so the pre-registered "better on everything" gate failed. Golden set by the Findex author |
| Intent router | dev 272/272; frozen held-out v2 **57/60** | v2 56/60 before the 2026-10-26 rules | `crates/findex-index/src/router_heldout_v2.tsv` + test | Proven on v2. v1 (60/60) was tuned on and no longer counts as held out |

Known false negatives of the owner rules (seen on held-out data, not tuned
on): import aliases (`use … as …`), trait methods called through an
implementing type, and field-chain receivers in files that name only another
owner. Remaining router v2 misses: "remove", "consequences", "breaking".

## Token use with Claude Code

| Claim | Number | Evidence | Verdict |
|---|---|---|---|
| Navigation tasks (definitions, callers), held-out, Haiku 4.5 | **−23.7 %** tokens per session; 12/12 correct | Run 4, 108 sessions: `validation/campaigns/vnext/CLAUDE_AB_RESULTS.md` | Proven for one model and one task class |
| All 12 tasks, Haiku 4.5 | −15.9 %; 36/36 correct | same | Proven for one model |
| Concept questions, cross-file traces, code changes | confidence intervals include zero, or Findex costs more | same | Not proven |
| Owner-rule build on task `t2` (tokio callers) | 4/4 correct; n = 2 per arm, one Findex session never called Findex | `validation/results/20261026T093056Z-6e0a1f6e209c/` | Supported (correctness only, no token claim) |
| Sonnet, Opus | — | — | Not run |
| Codex client (plugin loads, 8 tools, source-verified answer) | app-server native check PASS | `validation/results/20261026-codex-comparison/codex-native.json` | Proven (integration only) |
| Codex/Claude live comparison of three tools | 10 of 12 sessions used; invalid or blocked runs | same, `NOTE.md` | Not proven |

`validation/FINAL_VALIDATION_REPORT.md` (2026-10-12) says token savings were
not demonstrated. That referred to the earlier 36-session study, where Findex
was called in 3 of 18 sessions. Run 4 superseded it.

## Safety and reliability

| Claim | Evidence | Verdict |
|---|---|---|
| Live model tests bill only the subscription | `test_live_gate.py` (14 tests, in CI); canary session reported `apiKeySource: "none"` with the local proxy port closed (`validation/results/20261026-live-canary/`) | Proven |
| Removed or unreadable file text leaves the index and WAL | `removed_file_text_is_not_left_in_*`, `access_denied_file_is_removed_with_its_text`, `refresh_removes_access_denied_text_from_database_and_wal` | Proven |
| Index directory not readable by other local accounts | `index_directory_is_not_readable_by_other_local_accounts` | Proven (Windows) |
| MCP tool errors leak no local paths or engine text | `engine_errors_never_reach_tool_results` (mutation-checked) | Proven |
| Corrupt index fails closed | security suite: refused and preserved, rebuilt only after deletion | Proven |
| Threat model T1–T12 | `validation/campaigns/enterprise/THREAT_MODEL.md` | All PASS, except model obedience to injected text (T2) and unlisted secret shapes (T3) |
| Cloud-only OneDrive (Cloud Files) placeholders are never downloaded | real Cloud Files provider counting download requests: 3/3 downloaded before the fix, 0 after; a file made local is indexed, one sent back leaves the index (`validation/results/20261026-cloud-and-smb/`, in CI) | Proven |
| SMB network path | loopback `\\localhost\C$`: CLI index/search/context and MCP watcher (`examined: 0`) see edits, deletes and creates (same dir, in CI) | Proven for loopback SMB; a remote file server is not covered |
| Corrupt cache recovery | `findex forget` deletes only this project's cache; then `findex index` rebuilds (`hardening_offline.py`) | Proven |
| Cold index of VS Code 1.94 (6,877 files) | 14–16 s, from 251 s at start (`validation/campaigns/vnext/PERFORMANCE_RESULTS.md`) | Proven on this machine |

## Not done

- Typed multi-language relationship graph. Only Rust has a compiler oracle (SCIP); other languages use name-based call sites.
- Windows 10, and a remote (non-loopback) SMB file server.
- Authenticode signing (no certificate was purchased).
- Token savings for non-navigation tasks and for models other than Haiku.
- How often the model adopts Findex at scale (needs many live sessions).

## Are we on track?

Accuracy work since 2026-10-12 improved every frozen held-out number it
touched, and none regressed. Safety blockers A11 and A12 from the enterprise
campaign are closed. Live tests now run on the subscription only. A real
enterprise bug (indexing a OneDrive folder downloaded every cloud-only file)
was found and fixed with a regression check in CI, and SMB paths are covered.
Against two other code-navigation tools, Findex is the most accurate on
definition lookup but not the cheapest in tokens. Open release
blockers are the "Not done" list above. `ENTERPRISE_READY` stays false until
signing and the typed graph exist.
