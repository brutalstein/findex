# Evidence status

Living summary of what Findex has demonstrated, as of **2026-10-27**
(version 0.2.0 release candidate, not yet released). Every number points to a committed run under
`validation/results/` or a campaign record; campaign records themselves are
historical and are not rewritten. Machine: native Windows 11, NVMe.

Historical CI (all green for PR #34, run 40342169198): fmt, Clippy
`-D warnings`, all tests on Windows latest, Windows Server 2022 and Rust 1.85
(MSRV), release build, RustSec audit, `cargo deny` license/source policy, and
the offline harnesses (security, hardening, cloud-only placeholders, SMB,
plugin, live-gate).

Verdicts:
- **Proven**: measured on held-out data or against an independent oracle, frozen before the run.
- **Supported**: measured, but only on development data, a single model, or small n.
- **Not proven**: measured, and the claim did not hold.
- **Not run**: no measurement.

## 2026-10-27 release-candidate verification

| Claim | Fresh evidence | Verdict |
|---|---|---|
| Compact output preserves evidence and reduces Findex's output tokens | 115 exact roundtrips and unchanged source-hit lists vs pre-hardening binary; 369.01 → 263.83 mean tokens across all 115 (28.5% fewer); new 55 subset 312.96 → 222.07 (29.0% fewer) | Proven on these fixed navigation cases |
| New navigation comparison | New 55: file hit@1 Findex 54/55, codebase 47/55, Graphify 48/55; definition span@1 54/55, 40/55, 42/55 | Supported within author-created, four-language golden |
| Findex is the smallest response | New 55 aggregate: compact 222.07, codebase tree 240.31, Graphify raw 2663.47; all-115 margin over codebase only 0.95% | Supported on these aggregates; codebase is smaller on three of four corpora and historical 60 controls |
| Real Codex subscription navigation | Six valid sessions, two tasks per tool, all correct. Total input+output: Findex 149527, Graphify 217280, codebase 592385; cache counts reported | Supported small pilot; no general savings or code-edit claim |
| Native Windows release checks | 153 Rust tests on stable/MSRV; 34 offline Python tests; fmt, Clippy, release build, RustSec; 29/29 quick black-box security checks; native installed Codex fixture and compact response, Claude strict plugin validator, package/privacy/overview harnesses | See `validation/results/20261027-performance-release/` for exact scope |

Native scaling against the previous MSVC build: 10k-file median initial index
49.01 → 10.21 s (4.80x),
three paired fresh-directory repetitions. One 100k-file candidate run takes
106.16 s; excess-file rollback preserves the old snapshot.
The wide-body workload falls 140.97 → 16.12 ms median with identical full
responses. These are synthetic local measurements, not competitor or arbitrary
project guarantees. [Methods and resource limits](engineering/RELEASE_CANDIDATE_HARDENING_20261027.md#native-scaling-and-query-work).

Methods, raw/visible output distinction and all losses:
[`20261027-performance-comparison`](../validation/results/20261027-performance-comparison/NOTE.md),
[`20261027-codex-live`](../validation/results/20261027-codex-live/NOTE.md).
The 115-query comparison contains 60 historical controls, not 115 new independent
queries. The final encoding reuses the preliminary evaluation set; it is a
regression confirmation rather than a second blind held-out study. The native MCP comparison improves the competitor baseline (anchored
exact symbols and codebase tree) and avoids Graphify CLI startup. It does not
retroactively replace the older study below.

Long-line credential redaction now runs before excerpt truncation. Exact-symbol
verification caps report `truncated` instead of suggesting exhaustive absence.
Source-hit equivalence does not remove the documented caps or create typed
cross-language call resolution. The release checklist and remaining limits are
in [the dated hardening report](engineering/RELEASE_CANDIDATE_HARDENING_20261027.md).

## Accuracy

| Claim | Number | Baseline | Evidence | Verdict |
|---|---|---|---|---|
| File retrieval (642 golden queries, 7 corpora) | R@1 0.984, MRR 0.991; held-out R@1 0.974 | start build R@1 0.695; `git grep` R@1 0.790 | `validation/FINAL_VALIDATION_REPORT.md` §1 | Proven (golden set written by the engine author) |
| Owner-qualified callers, ripgrep (rust-analyzer SCIP oracle, held-out split) | call precision **0.714**, site recall **0.981** | 0.523 / 0.957 | `validation/results/20261026-callsites-qualified-final/` | Proven |
| Owner-qualified callers, tokio (same) | call precision **0.567**, site recall **0.947** | 0.376 / 0.829 | same | Proven |
| Unqualified callers, tokio (dev split) | call precision 0.418 | 0.391 | same | Supported |
| Coverage line ("every code occurrence is shown") vs `git grep -w` | 279/279 names, 7 corpora | 268/279 at frozen main (A1) | same, `coverage/` | Proven |
| Issue localization (65 real fixed issues, frozen) | `findex_context` hit@1 0.308, MRR 0.428, gold recall@10 0.662 | `git grep` words hit@1 0.062, MRR 0.253 | same, `localization/` | Proven better than `git grep`; hit@1 is still low |
| Definition lookup vs other tools (60 held-out queries, 4 corpora, same budget) | Findex hit@1 58/60 (0.967), MRR 0.983, mean returned 418.55 tokens; Graphify (code-only) 39/60 (0.650) / 0.676 / 1,466.07 clipped visible tokens (1,919.10 actually returned); codebase-memory-mcp 38/60 (0.633) / 0.646 / 354.63 returned tokens | — | `validation/results/20261026-codex-comparison/` | Supported: more accurate than both; fewer tokens than Graphify but not codebase-memory-mcp, so the pre-registered "better on everything" gate failed. Golden set by the Findex author; historical manifest lacks content/tool identity hashes |
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
| Codex/Claude live comparison of three tools | Six of 12 authorized attempts started cumulatively: four invalid permission attempts, one valid correct Findex answer, then Graphify exit 1 with no usage and a usage-limit stop; Claude blocked | same, `NOTE.md`, `live-results.json`; `live-invalid-permission-trials.json` is the overlapping four-attempt prefix | Not proven; incomplete usage cannot establish session-token totals or actual subscription billing |

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
