# Final decision

Branch `enterprise/p0-safety`, based on `main` `247576e`. Every number
below is in the linked result documents; there is no aggregate score.

## Release state

| State | Decision | Reason |
|---|---|---|
| DEVELOPER_READY | **Yes** | Local Windows gate passes (fmt, clippy `-D warnings`, all workspace tests, release build); offline validation cannot start a model client; coverage claim holds 279/279 against an independent checker; retrieval unchanged; security 30/30. |
| PRODUCTION_CANDIDATE | **No** | Blocking: CI on this branch not yet confirmed green (UNVERIFIED); MCP errors can carry absolute paths (A11, FAIL); router held-out accuracy 50/60 with callers 5/10; the new router/lexer/budget behaviour was not checked with a live model (NOT_RUN by policy); Windows 10, network volumes and cloud-only OneDrive files NOT_RUN. |
| ENTERPRISE_READY | **No** | Additionally: no Authenticode signature (BLOCKED, no certificate purchased); typed semantic graph not built (NOT_RUN); index directory ACL not tested (A12); relations database unversioned (A7); prompt-injection resistance of the model UNVERIFIED. |

## Done in this campaign

| Phase | Item | Status |
|---|---|---|
| P0 | Repository audit (`docs/engineering/COMPLETE_REPOSITORY_AUDIT.md`), file inventory, baseline freeze | PASS |
| P0 | Offline-by-default runner, fail-closed live-model gate, CI spawn test | PASS |
| P1 | Typed graph, ambiguous names, compiler-backed graph tests | NOT_RUN |
| P2 | Intent router with ≥ 250 EN/TR/mixed cases, file intent, frozen held-out set | PASS (dev 258/260, held-out 50/60) |
| P2 | Budgeted packing counts the whole report | PASS (0/240 reports over budget) |
| P2 | Comment/string lexer behind the coverage claim | PASS (279/279) |
| P3 | Locked files kept in refresh and full reconciliation | PASS |
| P3 | Newer schema refused; 3 → 4 migration | PASS |
| P4 | Removed text zeroed from the cache | PASS (main DB file); WAL limitation open |
| P4 | Docs: README, SECURITY, architecture, doctor | PASS |
| P4 | MCP error path redaction (A11), ACL test (A12) | FAIL / NOT_RUN |
| P4 | Official GitHub Release | NOT_RUN (needs approval) |
| P5 | ≤ 12-session live smoke study | NOT_RUN (needs approval; plan in MODEL_VALIDATION) |

## Model usage and cost

New live model sessions: 0. Paid API calls: 0. Paid infrastructure: 0.

## Next steps, in order

1. Confirm CI on the pushed branch; merge through a reviewed pull request.
2. Fix A11 (strip absolute paths from MCP error texts) with a test.
3. Add held-out-failing phrasings only to a *new* dev set, then write a
   fresh held-out set; never edit `router_heldout.tsv`.
4. With operator approval, run the ≤ 12-session smoke study.
5. P1 typed graph and relations schema versioning.

## Update 2026-10-26 (after this campaign)

The verdicts above are unchanged; this records what happened to their
blockers. Current numbers: [docs/EVIDENCE.md](../../../docs/EVIDENCE.md).

- Next step 2 (A11): closed. Fixed error texts, test `engine_errors_never_reach_tool_results`.
- A12: closed. Test `index_directory_is_not_readable_by_other_local_accounts`.
- Next step 3: done. Fresh held-out router set `router_heldout_v2.tsv`, frozen
  before its first run: 56/60, then 57/60 after dev-only rules.
- Next step 4 (live smoke): live sessions are gated per session on
  `apiKeySource: "none"`; one canary and a 4-session `t2` check ran on Haiku.
  The full 12-session study was not run.
- CI: two Windows-runner test failures (pwsh 7 module path, elevated runner
  reading an empty-DACL file) fixed in `21a8ab8` and `97d36d1`.
- Still open for ENTERPRISE_READY: Authenticode, typed graph, A7 remainder,
  model obedience to injected text.
- CI is green on the branch (`52d9a8a`, `e4fe732`).
- Windows reliability: cloud-only OneDrive placeholders were downloaded by
  indexing (fixed in `e4fe732`; 0 downloads after the fix, checked by a real
  Cloud Files provider). Loopback SMB passes, including the MCP watcher. Both
  run in CI. Windows 10 and remote SMB servers are still NOT_RUN.
- Release engineering (`ecc7f9a`): tag-driven GitHub Release after all
  gates, SBOM, SHA-256, provenance, license/source policy (`cargo deny`), a
  Windows Server 2022 job, and an Authenticode step that runs once a
  certificate secret exists. No release was published.
- Of the PRODUCTION_CANDIDATE blockers above, only Windows 10 and the
  12-session live study remain open.
